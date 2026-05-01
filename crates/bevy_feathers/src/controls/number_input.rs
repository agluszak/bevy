use bevy_app::{Plugin, PreUpdate, PropagateOver};
use bevy_asset::AssetServer;
use bevy_camera::visibility::Visibility;
use bevy_ecs::{
    component::Component,
    entity::Entity,
    event::EntityEvent,
    hierarchy::Children,
    observer::On,
    query::{Changed, With},
    schedule::IntoScheduleConfigs,
    system::Query,
    template::template,
};
use bevy_picking::{hover::Hovered, PickingSystems};
use bevy_scene::prelude::*;
use bevy_text::{FontSource, FontWeight, TextFont};
use bevy_ui::{
    px, widget::Text, AlignItems, AlignSelf, Display, FlexDirection, JustifyContent, Node, UiRect,
};
use bevy_ui_widgets::{
    NumberInput as CoreNumberInput, NumberSpinBoxValueInput, SelectAllOnFocus,
    SetNumberSpinBoxValue, SpinBox, SpinBoxDecrementButton, SpinBoxIncrementButton,
};

use crate::{
    constants::{fonts, size},
    controls::{
        button, text_input, text_input_container, ButtonProps, ButtonVariant, TextInputProps,
    },
    theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor, ThemeToken, ThemedText},
    tokens,
};

pub use bevy_ui_widgets::{NumberFormat, NumberInputValue};

/// Marker to indicate a number input widget with feathers styling.
#[derive(Component, Default, Clone)]
struct FeathersNumberInput;

/// Marker for the spinbox button container that should only be shown while hovered.
#[derive(Component, Default, Clone)]
struct FeathersSpinButtons;

/// Parameters for the text input template, passed to [`number_input`] function.
pub struct NumberInputProps {
    /// The "sigil" is a colored strip along the left edge of the input, which is used to
    /// distinguish between different axes. The default is transparent (no sigil).
    pub sigil_color: ThemeToken,
    /// A caption to be placed on the left side of the input, next to the colored stripe.
    /// Usually one of "X", "Y" or "Z".
    pub label_text: Option<&'static str>,
    /// Indicate what size numbers we are editing.
    pub number_format: NumberFormat,
}

impl Default for NumberInputProps {
    fn default() -> Self {
        Self {
            sigil_color: tokens::TEXT_INPUT_BG,
            label_text: None,
            number_format: NumberFormat::F32,
        }
    }
}

/// Event which can be sent to the styled Feathers number input to update the displayed value.
///
/// This preserves the existing Feathers API by targeting the outer styled widget entity, and
/// forwards internally to the wrapped core [`bevy_ui_widgets::SpinBox`].
#[derive(Clone, EntityEvent)]
pub struct UpdateNumberInput {
    /// Target widget.
    pub entity: Entity,

    /// Value to change to.
    pub value: NumberInputValue,
}

/// Styled Feathers wrapper around the core [`bevy_ui_widgets::SpinBox`].
///
/// The Feathers widget keeps only layout, theming, and hover-only button presentation. Numeric
/// editing, stepping, and typed value-change behavior are handled by the core widgets.
pub fn number_input(props: NumberInputProps) -> impl Scene {
    let number_format = props.number_format;

    bsn! {
        :text_input_container()
        ThemeBorderColor({props.sigil_color.clone()})
        FeathersNumberInput
        SpinBox
        Hovered
        on(number_input_on_update)
        Children [
            {
                match props.label_text {
                    Some(text) => Box::new(bsn_list!(
                        Node {
                            display: Display::Flex,
                            align_items: AlignItems::Center,
                            align_self: AlignSelf::Stretch,
                            justify_content: JustifyContent::Center,
                            padding: UiRect::axes(px(6), px(0)),
                        }
                        ThemeBackgroundColor(tokens::TEXT_INPUT_LABEL_BG)
                        Children [
                            Text::new(text.to_string())
                            template(|ctx| {
                                Ok(TextFont {
                                    font: FontSource::Handle(ctx.resource::<AssetServer>().load(fonts::REGULAR)),
                                    font_size: size::COMPACT_FONT,
                                    weight: FontWeight::NORMAL,
                                    ..Default::default()
                                })
                            })
                            PropagateOver<TextFont>
                            ThemeTextColor(tokens::TEXT_INPUT_TEXT)
                        ]
                    )) as Box<dyn SceneList>,
                    None => Box::new(bsn_list!()) as Box<dyn SceneList>
                }
            },
            (
                text_input(TextInputProps {
                    visible_width: None,
                    max_characters: Some(20),
                })
                CoreNumberInput {
                    format: number_format,
                }
                NumberSpinBoxValueInput
                SelectAllOnFocus
            ),
            (
                Node {
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_self: AlignSelf::Stretch,
                    justify_content: JustifyContent::Center,
                    width: px(18),
                    row_gap: px(1),
                }
                template_value(Visibility::Hidden)
                FeathersSpinButtons
                Children [
                    (
                        button(ButtonProps {
                            caption: Box::new(bsn_list!((Text("+") ThemedText))),
                            variant: ButtonVariant::Plain,
                            ..Default::default()
                        })
                        SpinBoxIncrementButton
                        Node {
                            min_width: px(18),
                            height: px(11),
                            padding: UiRect::axes(px(0), px(0)),
                        }
                    ),
                    (
                        button(ButtonProps {
                            caption: Box::new(bsn_list!((Text("-") ThemedText))),
                            variant: ButtonVariant::Plain,
                            ..Default::default()
                        })
                        SpinBoxDecrementButton
                        Node {
                            min_width: px(18),
                            height: px(11),
                            padding: UiRect::axes(px(0), px(0)),
                        }
                    ),
                ]
            ),
        ]
    }
}

fn number_input_on_update(
    update: On<UpdateNumberInput>,
    q_feathers: Query<(), With<FeathersNumberInput>>,
    mut commands: bevy_ecs::system::Commands,
) {
    if !q_feathers.contains(update.event_target()) {
        return;
    }

    commands.trigger(SetNumberSpinBoxValue {
        entity: update.event_target(),
        value: update.value,
    });
}

fn update_spinbox_button_visibility(
    q_spinboxes: Query<(Entity, &Hovered), (With<FeathersNumberInput>, Changed<Hovered>)>,
    q_children: Query<&Children>,
    mut q_button_groups: Query<&mut Visibility, With<FeathersSpinButtons>>,
) {
    for (spinbox, hovered) in q_spinboxes.iter() {
        let visibility = if hovered.0 {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };

        for child in q_children.iter_descendants(spinbox) {
            if let Ok(mut button_visibility) = q_button_groups.get_mut(child) {
                *button_visibility = visibility;
            }
        }
    }
}

/// Plugin which registers the systems for updating Feathers number-input styling.
pub struct NumberInputPlugin;

impl Plugin for NumberInputPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(
            PreUpdate,
            update_spinbox_button_visibility.in_set(PickingSystems::Last),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_ecs::{observer::On, prelude::*};

    #[derive(Resource, Default)]
    struct ForwardedSpinBoxUpdates(Vec<(Entity, NumberInputValue)>);

    #[test]
    fn update_number_input_forwards_to_core_spinbox_update() {
        let mut app = App::new();
        app.init_resource::<ForwardedSpinBoxUpdates>();

        let root = app
            .world_mut()
            .spawn((FeathersNumberInput, SpinBox))
            .observe(number_input_on_update)
            .observe(
                |update: On<SetNumberSpinBoxValue>,
                 mut forwarded: ResMut<ForwardedSpinBoxUpdates>| {
                    forwarded.0.push((update.event_target(), update.value));
                },
            )
            .id();

        app.world_mut().commands().trigger(UpdateNumberInput {
            entity: root,
            value: NumberInputValue::I32(9),
        });
        app.update();

        assert_eq!(
            app.world().resource::<ForwardedSpinBoxUpdates>().0,
            vec![(root, NumberInputValue::I32(9))]
        );
    }

    #[test]
    fn hover_changes_toggle_spin_button_visibility() {
        let mut app = App::new();
        app.add_plugins(NumberInputPlugin);

        let root = app
            .world_mut()
            .spawn((FeathersNumberInput, Hovered(false)))
            .id();
        let button_group = app
            .world_mut()
            .spawn((FeathersSpinButtons, Visibility::Hidden, ChildOf(root)))
            .id();

        app.world_mut().entity_mut(root).insert(Hovered(true));
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(button_group),
            Some(&Visibility::Visible)
        );

        app.world_mut().entity_mut(root).insert(Hovered(false));
        app.update();

        assert_eq!(
            app.world().get::<Visibility>(button_group),
            Some(&Visibility::Hidden)
        );
    }
}
