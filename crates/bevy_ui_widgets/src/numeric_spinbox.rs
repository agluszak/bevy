use bevy_app::{App, Plugin};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    event::EntityEvent,
    hierarchy::{ChildOf, Children},
    observer::On,
    query::With,
    system::{Commands, Query, Res},
};
use bevy_input_focus::InputFocus;
use bevy_log::warn;
use bevy_text::EditableText;

use crate::{
    number_input::{
        queue_number_input_value_update, queue_number_input_value_update_if_unfocused, NumberInput,
        NumberInputParseError, NumberInputValue,
    },
    spinbox::find_spinbox_ancestor,
    SpinBox, SpinBoxButtonPress, SpinBoxDirection, ValueChange,
};

/// Marks the descendant [`NumberInput`] used as the numeric value field of a [`SpinBox`].
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(NumberInput)]
pub struct NumberSpinBoxValueInput;

/// Optional custom numeric step size for a [`SpinBox`].
#[derive(Component, Debug, Clone, Copy)]
pub struct NumberSpinBoxStep(pub NumberInputValue);

/// Programmatically replace the displayed value of a numeric spinbox.
///
/// Like [`crate::SetNumberInputValue`], the update is ignored if the embedded number input
/// currently has focus.
#[derive(Clone, EntityEvent)]
pub struct SetNumberSpinBoxValue {
    /// The target spinbox.
    #[event_target]
    pub entity: Entity,
    /// The value to display.
    pub value: NumberInputValue,
}

fn find_number_spinbox_value_input(
    spinbox: Entity,
    q_children: &Query<&Children>,
    mut has_value_input: impl FnMut(Entity) -> bool,
) -> Option<Entity> {
    q_children
        .iter_descendants(spinbox)
        .find(|child| has_value_input(*child))
}

fn number_spinbox_on_button_press(
    press: On<SpinBoxButtonPress>,
    q_spinbox: Query<Option<&NumberSpinBoxStep>, With<SpinBox>>,
    q_children: Query<&Children>,
    mut q_value_input: Query<
        (Entity, &NumberInput, &mut EditableText),
        With<NumberSpinBoxValueInput>,
    >,
    mut commands: Commands,
) {
    let Some(input_entity) = find_number_spinbox_value_input(press.entity, &q_children, |child| {
        q_value_input.contains(child)
    }) else {
        return;
    };

    let Ok((_, number_input, mut editable_text)) = q_value_input.get_mut(input_entity) else {
        return;
    };

    let current_value = match number_input
        .format
        .parse(&editable_text.value().to_string())
    {
        Ok(value) => value,
        Err(NumberInputParseError::Empty) => {
            warn!("SpinBox cannot step an empty numeric input");
            return;
        }
        Err(error) => {
            warn!("SpinBox cannot step invalid input: {error}");
            return;
        }
    };

    let step = q_spinbox
        .get(press.entity)
        .ok()
        .flatten()
        .map(|step| step.0)
        .unwrap_or(NumberInputValue::one_for(number_input.format));

    let next_value =
        match current_value.adjust(step, press.direction == SpinBoxDirection::Increment) {
            Ok(value) => value,
            Err(error) => {
                warn!("SpinBox step ignored: {error}");
                return;
            }
        };

    queue_number_input_value_update(input_entity, next_value, &mut editable_text, &mut commands);
    next_value.emit(press.entity, true, &mut commands);
}

fn number_spinbox_on_set_value(
    set_value: On<SetNumberSpinBoxValue>,
    q_spinbox: Query<(), With<SpinBox>>,
    q_children: Query<&Children>,
    q_value_input: Query<Entity, With<NumberSpinBoxValueInput>>,
    mut q_number_input: Query<&mut EditableText, With<NumberInput>>,
    focus: Option<Res<InputFocus>>,
    mut commands: Commands,
) {
    if !q_spinbox.contains(set_value.event_target()) {
        return;
    }

    let Some(input_entity) =
        find_number_spinbox_value_input(set_value.event_target(), &q_children, |child| {
            q_value_input.contains(child)
        })
    else {
        return;
    };

    queue_number_input_value_update_if_unfocused(
        input_entity,
        set_value.value,
        &mut q_number_input,
        focus.as_deref(),
        &mut commands,
    );
}

macro_rules! number_spinbox_forward_value_change {
    ($name:ident, $ty:ty) => {
        fn $name(
            change: On<ValueChange<$ty>>,
            q_value_input: Query<(), With<NumberSpinBoxValueInput>>,
            q_parent: Query<&ChildOf>,
            q_spinbox: Query<(), With<SpinBox>>,
            mut commands: Commands,
        ) {
            if !q_value_input.contains(change.source) {
                return;
            }

            let Some(spinbox) = find_spinbox_ancestor(change.source, &q_parent, &q_spinbox) else {
                return;
            };

            commands.trigger(ValueChange {
                source: spinbox,
                value: change.value,
                is_final: change.is_final,
            });
        }
    };
}

number_spinbox_forward_value_change!(number_spinbox_on_number_change_f32, f32);
number_spinbox_forward_value_change!(number_spinbox_on_number_change_f64, f64);
number_spinbox_forward_value_change!(number_spinbox_on_number_change_i32, i32);
number_spinbox_forward_value_change!(number_spinbox_on_number_change_i64, i64);

/// Plugin that adds observers for the composed numeric [`SpinBox`] behavior.
pub struct NumericSpinBoxPlugin;

impl Plugin for NumericSpinBoxPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(number_spinbox_on_button_press)
            .add_observer(number_spinbox_on_set_value)
            .add_observer(number_spinbox_on_number_change_f32)
            .add_observer(number_spinbox_on_number_change_f64)
            .add_observer(number_spinbox_on_number_change_i32)
            .add_observer(number_spinbox_on_number_change_i64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_ecs::{observer::On, prelude::*};
    use bevy_input::{
        keyboard::{Key, KeyCode, KeyboardInput},
        ButtonState, InputPlugin,
    };
    use bevy_input_focus::{InputDispatchPlugin, InputFocus, InputFocusPlugin};
    use bevy_window::{PrimaryWindow, Window};

    use crate::{
        number_input::{IgnoreNextNumberInputChange, NumberInputAdjustError},
        NumberFormat,
    };

    #[derive(Resource, Default)]
    struct NumberSpinBoxEvents(Vec<(Entity, i32, bool)>);

    fn keyboard_input(key_code: KeyCode) -> KeyboardInput {
        KeyboardInput {
            key_code,
            logical_key: match key_code {
                KeyCode::ArrowUp => Key::ArrowUp,
                KeyCode::ArrowDown => Key::ArrowDown,
                KeyCode::ArrowLeft => Key::ArrowLeft,
                KeyCode::ArrowRight => Key::ArrowRight,
                _ => unreachable!(),
            },
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    #[test]
    fn integer_adjust_rejects_float_steps() {
        assert_eq!(
            NumberInputValue::I32(3).adjust(NumberInputValue::F32(0.5), true),
            Err(NumberInputAdjustError::IntegerStepRequired)
        );
    }

    #[test]
    fn set_number_spinbox_value_marks_unfocused_embedded_input() {
        let mut app = App::new();
        app.add_plugins((crate::NumberInputPlugin, NumericSpinBoxPlugin));

        let spinbox = app.world_mut().spawn(SpinBox).id();
        let value_input = app
            .world_mut()
            .spawn((
                NumberInput {
                    format: NumberFormat::I32,
                },
                NumberSpinBoxValueInput,
                EditableText::new("4"),
                ChildOf(spinbox),
            ))
            .id();

        app.world_mut().commands().trigger(SetNumberSpinBoxValue {
            entity: spinbox,
            value: NumberInputValue::I32(8),
        });
        app.update();

        assert!(app
            .world()
            .entity(value_input)
            .contains::<IgnoreNextNumberInputChange>());
    }

    #[test]
    fn set_number_spinbox_value_ignores_focused_embedded_input() {
        let mut app = App::new();
        app.add_plugins((crate::NumberInputPlugin, NumericSpinBoxPlugin));

        let spinbox = app.world_mut().spawn(SpinBox).id();
        let value_input = app
            .world_mut()
            .spawn((
                NumberInput {
                    format: NumberFormat::I32,
                },
                NumberSpinBoxValueInput,
                EditableText::new("4"),
                ChildOf(spinbox),
            ))
            .id();
        app.world_mut()
            .insert_resource(InputFocus::from_entity(value_input));

        app.world_mut().commands().trigger(SetNumberSpinBoxValue {
            entity: spinbox,
            value: NumberInputValue::I32(8),
        });
        app.update();

        assert!(!app
            .world()
            .entity(value_input)
            .contains::<IgnoreNextNumberInputChange>());
    }

    #[test]
    fn number_spinbox_steps_and_emits_from_root() {
        let mut app = App::new();
        app.init_resource::<NumberSpinBoxEvents>()
            .add_plugins((
                crate::ButtonPlugin,
                crate::SpinBoxPlugin,
                crate::NumberInputPlugin,
                NumericSpinBoxPlugin,
            ))
            .add_observer(
                |change: On<ValueChange<i32>>, mut events: ResMut<NumberSpinBoxEvents>| {
                    events
                        .0
                        .push((change.source, change.value, change.is_final));
                },
            );

        let spinbox = app.world_mut().spawn(SpinBox).id();
        let value_input = app
            .world_mut()
            .spawn((
                NumberInput {
                    format: NumberFormat::I32,
                },
                NumberSpinBoxValueInput,
                EditableText::new("4"),
                ChildOf(spinbox),
            ))
            .id();
        let increment = app
            .world_mut()
            .spawn((crate::SpinBoxIncrementButton, ChildOf(spinbox)))
            .id();

        app.world_mut()
            .commands()
            .trigger(crate::Activate { entity: increment });
        app.update();

        assert_eq!(
            app.world().resource::<NumberSpinBoxEvents>().0,
            vec![(spinbox, 5, true)]
        );
        assert!(app.world().entity(value_input).contains::<NumberInput>());
    }

    #[test]
    fn number_spinbox_forwards_number_input_changes() {
        let mut app = App::new();
        app.init_resource::<NumberSpinBoxEvents>()
            .add_plugins((crate::SpinBoxPlugin, NumericSpinBoxPlugin))
            .add_observer(
                |change: On<ValueChange<i32>>, mut events: ResMut<NumberSpinBoxEvents>| {
                    events
                        .0
                        .push((change.source, change.value, change.is_final));
                },
            );

        let spinbox = app.world_mut().spawn(SpinBox).id();
        let value_input = app
            .world_mut()
            .spawn((
                NumberInput {
                    format: NumberFormat::I32,
                },
                NumberSpinBoxValueInput,
                EditableText::new("7"),
                ChildOf(spinbox),
            ))
            .id();

        app.world_mut().commands().trigger(ValueChange {
            source: value_input,
            value: 8_i32,
            is_final: false,
        });
        app.update();

        assert!(app
            .world()
            .resource::<NumberSpinBoxEvents>()
            .0
            .contains(&(spinbox, 8, false)));
    }

    #[test]
    fn number_spinbox_forwards_nested_number_input_changes() {
        let mut app = App::new();
        app.init_resource::<NumberSpinBoxEvents>()
            .add_plugins((crate::SpinBoxPlugin, NumericSpinBoxPlugin))
            .add_observer(
                |change: On<ValueChange<i32>>, mut events: ResMut<NumberSpinBoxEvents>| {
                    events
                        .0
                        .push((change.source, change.value, change.is_final));
                },
            );

        let spinbox = app.world_mut().spawn(SpinBox).id();
        let wrapper = app.world_mut().spawn(ChildOf(spinbox)).id();
        let value_input = app
            .world_mut()
            .spawn((
                NumberInput {
                    format: NumberFormat::I32,
                },
                NumberSpinBoxValueInput,
                EditableText::new("7"),
                ChildOf(wrapper),
            ))
            .id();

        app.world_mut().commands().trigger(ValueChange {
            source: value_input,
            value: 8_i32,
            is_final: false,
        });
        app.update();

        assert!(app
            .world()
            .resource::<NumberSpinBoxEvents>()
            .0
            .contains(&(spinbox, 8, false)));
    }

    #[test]
    fn number_spinbox_uses_custom_step_value() {
        let mut app = App::new();
        app.init_resource::<NumberSpinBoxEvents>()
            .add_plugins((
                crate::ButtonPlugin,
                crate::SpinBoxPlugin,
                crate::NumberInputPlugin,
                NumericSpinBoxPlugin,
            ))
            .add_observer(
                |change: On<ValueChange<i32>>, mut events: ResMut<NumberSpinBoxEvents>| {
                    events
                        .0
                        .push((change.source, change.value, change.is_final));
                },
            );

        let spinbox = app
            .world_mut()
            .spawn((SpinBox, NumberSpinBoxStep(NumberInputValue::I32(2))))
            .id();
        let _value_input = app.world_mut().spawn((
            NumberInput {
                format: NumberFormat::I32,
            },
            NumberSpinBoxValueInput,
            EditableText::new("4"),
            ChildOf(spinbox),
        ));
        let increment = app
            .world_mut()
            .spawn((crate::SpinBoxIncrementButton, ChildOf(spinbox)))
            .id();

        app.world_mut()
            .commands()
            .trigger(crate::Activate { entity: increment });
        app.update();

        assert_eq!(
            app.world().resource::<NumberSpinBoxEvents>().0,
            vec![(spinbox, 6, true)]
        );
    }

    #[test]
    fn focused_number_input_arrow_key_steps_spinbox() {
        let mut app = App::new();
        app.init_resource::<NumberSpinBoxEvents>()
            .init_resource::<bevy_ui::UiScale>()
            .add_message::<bevy_window::Ime>()
            .add_message::<bevy_picking::events::Pointer<bevy_picking::events::Release>>()
            .add_plugins((
                InputPlugin,
                InputFocusPlugin,
                InputDispatchPlugin,
                crate::EditableTextInputPlugin,
                crate::SpinBoxPlugin,
                crate::NumberInputPlugin,
                NumericSpinBoxPlugin,
            ))
            .add_observer(
                |change: On<ValueChange<i32>>, mut events: ResMut<NumberSpinBoxEvents>| {
                    events
                        .0
                        .push((change.source, change.value, change.is_final));
                },
            );
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.update();

        let spinbox = app.world_mut().spawn(SpinBox).id();
        let value_input = app
            .world_mut()
            .spawn((
                NumberInput {
                    format: NumberFormat::I32,
                },
                NumberSpinBoxValueInput,
                EditableText::new("4"),
                ChildOf(spinbox),
            ))
            .id();
        app.world_mut()
            .insert_resource(InputFocus::from_entity(value_input));

        app.world_mut()
            .write_message(keyboard_input(KeyCode::ArrowRight))
            .unwrap();
        app.update();

        assert!(app
            .world()
            .resource::<NumberSpinBoxEvents>()
            .0
            .contains(&(spinbox, 5, true)));
    }
}
