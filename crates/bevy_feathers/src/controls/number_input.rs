use bevy_app::PropagateOver;
use bevy_asset::AssetServer;
use bevy_ecs::{
    component::Component, entity::Entity, event::EntityEvent, hierarchy::Children, observer::On,
    query::With, system::Query, template::template,
};
use bevy_scene::prelude::*;
use bevy_text::{EditableText, FontSource, FontWeight, TextFont};
use bevy_ui::{px, widget::Text, AlignItems, AlignSelf, Display, JustifyContent, Node, UiRect};
use bevy_ui_widgets::{
    NumberInput as CoreNumberInput, NumberScrubber, NumberScrubberValueInput, RangedNumberInput,
    RangedNumberInputValueInput, SelectAllOnFocus, SetNumberInputValue, SetNumberScrubberValue,
    SetRangedNumberInputValue, SliderOrientation, SliderPrecision, SliderRange, SliderValue,
    TrackClick,
};

use crate::{
    constants::{fonts, size},
    controls::{text_input, text_input_container, TextInputProps},
    theme::{ThemeBackgroundColor, ThemeBorderColor, ThemeTextColor, ThemeToken},
    tokens,
};

pub use bevy_ui_widgets::{NumberFormat, NumberInputValue};

/// Marker to indicate a number input widget with Feathers styling.
#[derive(Component, Default, Clone)]
struct FeathersNumberInput;

/// Marker to indicate a Feathers ranged number input.
#[derive(Component, Default, Clone)]
struct FeathersRangedNumberInput;

/// Marker to indicate a Feathers number scrubber.
#[derive(Component, Default, Clone)]
struct FeathersNumberScrubber;

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

/// Parameters for [`ranged_number_input`].
pub struct RangedNumberInputProps {
    /// Current value.
    pub value: f32,
    /// Minimum value.
    pub min: f32,
    /// Maximum value.
    pub max: f32,
    /// Decimal precision displayed after drag/programmatic updates.
    pub precision: i32,
    /// Optional colored left strip.
    pub sigil_color: ThemeToken,
    /// Optional axis/caption label.
    pub label_text: Option<&'static str>,
}

impl Default for RangedNumberInputProps {
    fn default() -> Self {
        Self {
            value: 0.0,
            min: 0.0,
            max: 1.0,
            precision: 3,
            sigil_color: tokens::TEXT_INPUT_BG,
            label_text: None,
        }
    }
}

/// Parameters for [`number_scrubber`].
pub struct NumberScrubberProps {
    /// Current value.
    pub value: NumberInputValue,
    /// Number format.
    pub number_format: NumberFormat,
    /// Optional colored left strip.
    pub sigil_color: ThemeToken,
    /// Optional axis/caption label.
    pub label_text: Option<&'static str>,
}

impl Default for NumberScrubberProps {
    fn default() -> Self {
        Self {
            value: NumberInputValue::F32(0.0),
            number_format: NumberFormat::F32,
            sigil_color: tokens::TEXT_INPUT_BG,
            label_text: None,
        }
    }
}

/// Event which can be sent to the styled Feathers number input to update the displayed value.
#[derive(Clone, EntityEvent)]
pub struct UpdateNumberInput {
    /// Target widget.
    #[event_target]
    pub entity: Entity,

    /// Value to change to.
    pub value: NumberInputValue,
}

/// Event sent to the styled Feathers ranged number input to update the displayed value.
#[derive(Clone, EntityEvent)]
pub struct UpdateRangedNumberInput {
    /// Target widget.
    #[event_target]
    pub entity: Entity,
    /// Value to change to.
    pub value: f32,
}

/// Event sent to the styled Feathers number scrubber to update the displayed value.
#[derive(Clone, EntityEvent)]
pub struct UpdateNumberScrubber {
    /// Target widget.
    #[event_target]
    pub entity: Entity,
    /// Value to change to.
    pub value: NumberInputValue,
}

/// Styled Feathers wrapper around the core [`bevy_ui_widgets::NumberInput`].
///
/// Numeric parsing, filtering, and typed value-change behavior are handled by the core widget.
pub fn number_input(props: NumberInputProps) -> impl Scene {
    let number_format = props.number_format;

    bsn! {
        :text_input_container()
        ThemeBorderColor({props.sigil_color.clone()})
        FeathersNumberInput
        on(number_input_on_update)
        Children [
            { number_label(props.label_text) },
            (
                text_input(TextInputProps {
                    visible_width: None,
                    max_characters: Some(20),
                })
                CoreNumberInput {
                    format: number_format,
                }
                SelectAllOnFocus
            ),
        ]
    }
}

fn number_label(label_text: Option<&'static str>) -> Box<dyn SceneList> {
    match label_text {
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
        None => Box::new(bsn_list!()) as Box<dyn SceneList>,
    }
}

/// Styled Feathers wrapper for a bounded, whole-field draggable numeric input.
pub fn ranged_number_input(props: RangedNumberInputProps) -> impl Scene {
    bsn! {
        :text_input_container()
        ThemeBorderColor({props.sigil_color.clone()})
        FeathersRangedNumberInput
        RangedNumberInput
        SliderValue({props.value})
        SliderRange::new(props.min, props.max)
        SliderPrecision({props.precision})
        bevy_ui_widgets::Slider {
            track_click: TrackClick::Drag,
            orientation: SliderOrientation::Horizontal,
        }
        on(ranged_number_input_on_update)
        Children [
            { number_label(props.label_text) },
            (
                text_input(TextInputProps {
                    visible_width: None,
                    max_characters: Some(20),
                })
                CoreNumberInput {
                    format: NumberFormat::F32,
                }
                RangedNumberInputValueInput
                SelectAllOnFocus
                EditableText::new(props.value.to_string())
            ),
        ]
    }
}

/// Styled Feathers wrapper for an unbounded whole-field numeric scrubber.
pub fn number_scrubber(props: NumberScrubberProps) -> impl Scene {
    let number_format = props.number_format;
    bsn! {
        :text_input_container()
        ThemeBorderColor({props.sigil_color.clone()})
        FeathersNumberScrubber
        NumberScrubber
        on(number_scrubber_on_update)
        Children [
            { number_label(props.label_text) },
            (
                text_input(TextInputProps {
                    visible_width: None,
                    max_characters: Some(20),
                })
                CoreNumberInput {
                    format: number_format,
                }
                NumberScrubberValueInput
                SelectAllOnFocus
                EditableText::new(props.value.to_string())
            ),
        ]
    }
}

fn number_input_on_update(
    update: On<UpdateNumberInput>,
    q_feathers: Query<(), With<FeathersNumberInput>>,
    q_children: Query<&Children>,
    q_core_input: Query<(), With<CoreNumberInput>>,
    mut commands: bevy_ecs::system::Commands,
) {
    if !q_feathers.contains(update.event_target()) {
        return;
    }

    for child in q_children.iter_descendants(update.event_target()) {
        if q_core_input.contains(child) {
            commands.trigger(SetNumberInputValue {
                entity: child,
                value: update.value,
            });
            break;
        }
    }
}

fn ranged_number_input_on_update(
    update: On<UpdateRangedNumberInput>,
    q_feathers: Query<(), With<FeathersRangedNumberInput>>,
    mut commands: bevy_ecs::system::Commands,
) {
    if !q_feathers.contains(update.event_target()) {
        return;
    }

    commands.trigger(SetRangedNumberInputValue {
        entity: update.event_target(),
        value: update.value,
    });
}

fn number_scrubber_on_update(
    update: On<UpdateNumberScrubber>,
    q_feathers: Query<(), With<FeathersNumberScrubber>>,
    mut commands: bevy_ecs::system::Commands,
) {
    if !q_feathers.contains(update.event_target()) {
        return;
    }

    commands.trigger(SetNumberScrubberValue {
        entity: update.event_target(),
        value: update.value,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_ecs::{observer::On, prelude::*};

    #[derive(Resource, Default)]
    struct ForwardedNumberUpdates(Vec<(Entity, NumberInputValue)>);

    #[derive(Resource, Default)]
    struct ForwardedRangedUpdates(Vec<(Entity, f32)>);

    #[derive(Resource, Default)]
    struct ForwardedScrubberUpdates(Vec<(Entity, NumberInputValue)>);

    #[test]
    fn update_number_input_forwards_to_core_update() {
        let mut app = App::new();
        app.init_resource::<ForwardedNumberUpdates>().add_observer(
            |update: On<SetNumberInputValue>, mut forwarded: ResMut<ForwardedNumberUpdates>| {
                forwarded.0.push((update.event_target(), update.value));
            },
        );

        let root = app
            .world_mut()
            .spawn(FeathersNumberInput)
            .observe(number_input_on_update)
            .id();
        let input = app
            .world_mut()
            .spawn((CoreNumberInput::default(), ChildOf(root)))
            .id();

        app.world_mut().commands().trigger(UpdateNumberInput {
            entity: root,
            value: NumberInputValue::I32(9),
        });
        app.update();

        assert_eq!(
            app.world().resource::<ForwardedNumberUpdates>().0,
            vec![(input, NumberInputValue::I32(9))]
        );
    }

    #[test]
    fn update_ranged_number_input_forwards_to_core_update() {
        let mut app = App::new();
        app.init_resource::<ForwardedRangedUpdates>();

        let root = app
            .world_mut()
            .spawn(FeathersRangedNumberInput)
            .observe(ranged_number_input_on_update)
            .observe(
                |update: On<SetRangedNumberInputValue>,
                 mut forwarded: ResMut<ForwardedRangedUpdates>| {
                    forwarded.0.push((update.event_target(), update.value));
                },
            )
            .id();

        app.world_mut().commands().trigger(UpdateRangedNumberInput {
            entity: root,
            value: 0.75,
        });
        app.update();

        assert_eq!(
            app.world().resource::<ForwardedRangedUpdates>().0,
            vec![(root, 0.75)]
        );
    }

    #[test]
    fn update_number_scrubber_forwards_to_core_update() {
        let mut app = App::new();
        app.init_resource::<ForwardedScrubberUpdates>();

        let root = app
            .world_mut()
            .spawn(FeathersNumberScrubber)
            .observe(number_scrubber_on_update)
            .observe(
                |update: On<SetNumberScrubberValue>,
                 mut forwarded: ResMut<ForwardedScrubberUpdates>| {
                    forwarded.0.push((update.event_target(), update.value));
                },
            )
            .id();

        app.world_mut().commands().trigger(UpdateNumberScrubber {
            entity: root,
            value: NumberInputValue::F32(2.5),
        });
        app.update();

        assert_eq!(
            app.world().resource::<ForwardedScrubberUpdates>().0,
            vec![(root, NumberInputValue::F32(2.5))]
        );
    }
}
