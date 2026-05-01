use bevy_app::{App, Plugin};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    event::EntityEvent,
    lifecycle::Insert,
    observer::On,
    query::{Has, With},
    system::{Commands, Query, Res},
    world::DeferredWorld,
};
use bevy_input::keyboard::{KeyCode, KeyboardInput};
use bevy_input::ButtonState;
use bevy_input_focus::{FocusLost, FocusedInput, InputFocus};
use bevy_log::warn;
use bevy_text::{EditableText, EditableTextFilter, TextEdit, TextEditChange};

use crate::ValueChange;

/// Defines what numeric type a [`NumberInput`] edits.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy)]
pub enum NumberFormat {
    /// A 32-bit float.
    #[default]
    F32,
    /// A 64-bit float.
    F64,
    /// A 32-bit integer.
    I32,
    /// A 64-bit integer.
    I64,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum NumberInputParseError {
    Empty,
    InvalidFloat,
    InvalidInteger,
}

impl core::fmt::Display for NumberInputParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            NumberInputParseError::Empty => write!(f, "empty numeric input"),
            NumberInputParseError::InvalidFloat => write!(f, "invalid floating-point number"),
            NumberInputParseError::InvalidInteger => write!(f, "invalid integer number"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum NumberInputAdjustError {
    IntegerStepRequired,
    StepOutOfRangeForI32,
    Overflow,
}

impl core::fmt::Display for NumberInputAdjustError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            NumberInputAdjustError::IntegerStepRequired => {
                write!(f, "integer number inputs require integer step values")
            }
            NumberInputAdjustError::StepOutOfRangeForI32 => {
                write!(f, "numeric step does not fit in i32")
            }
            NumberInputAdjustError::Overflow => {
                write!(f, "numeric value overflowed during step")
            }
        }
    }
}

impl NumberFormat {
    pub(crate) fn parse(self, text: &str) -> Result<NumberInputValue, NumberInputParseError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(NumberInputParseError::Empty);
        }

        match self {
            NumberFormat::F32 => text
                .parse::<f32>()
                .map(NumberInputValue::F32)
                .map_err(|_| NumberInputParseError::InvalidFloat),
            NumberFormat::F64 => text
                .parse::<f64>()
                .map(NumberInputValue::F64)
                .map_err(|_| NumberInputParseError::InvalidFloat),
            NumberFormat::I32 => text
                .parse::<i32>()
                .map(NumberInputValue::I32)
                .map_err(|_| NumberInputParseError::InvalidInteger),
            NumberFormat::I64 => text
                .parse::<i64>()
                .map(NumberInputValue::I64)
                .map_err(|_| NumberInputParseError::InvalidInteger),
        }
    }
}

/// Headless numeric text input.
///
/// This is the numeric-specialized layer on top of [`EditableText`]. The widget keeps the text
/// buffer local to the input entity, but emits typed [`ValueChange`] events for external state
/// management.
///
/// Add this component to the same entity as [`EditableText`]. A numeric character filter is
/// inserted automatically if the entity does not already have an [`EditableTextFilter`].
///
/// ```ignore
/// use bevy_ecs::prelude::*;
/// use bevy_text::{EditableText, TextCursorStyle};
/// use bevy_ui_widgets::{NumberFormat, NumberInput, SelectAllOnFocus, ValueChange};
///
/// commands.spawn((
///     NumberInput {
///         format: NumberFormat::F32,
///     },
///     EditableText::new("1.0"),
///     TextCursorStyle::default(),
///     SelectAllOnFocus,
/// )).observe(|change: On<ValueChange<f32>>| {
///     info!("new number: {}", change.value);
/// });
/// ```
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(EditableText)]
pub struct NumberInput {
    /// The numeric type edited by this input.
    pub format: NumberFormat,
}

/// Represents a concrete numeric value for programmatic updates and spinbox stepping.
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum NumberInputValue {
    /// An `f32` value.
    F32(f32),
    /// An `f64` value.
    F64(f64),
    /// An `i32` value.
    I32(i32),
    /// An `i64` value.
    I64(i64),
}

impl NumberInputValue {
    pub(crate) fn one_for(format: NumberFormat) -> Self {
        match format {
            NumberFormat::F32 => Self::F32(1.0),
            NumberFormat::F64 => Self::F64(1.0),
            NumberFormat::I32 => Self::I32(1),
            NumberFormat::I64 => Self::I64(1),
        }
    }

    pub(crate) fn emit(self, source: Entity, is_final: bool, commands: &mut Commands) {
        match self {
            NumberInputValue::F32(value) => {
                commands.trigger(ValueChange {
                    source,
                    value,
                    is_final,
                });
            }
            NumberInputValue::F64(value) => {
                commands.trigger(ValueChange {
                    source,
                    value,
                    is_final,
                });
            }
            NumberInputValue::I32(value) => {
                commands.trigger(ValueChange {
                    source,
                    value,
                    is_final,
                });
            }
            NumberInputValue::I64(value) => {
                commands.trigger(ValueChange {
                    source,
                    value,
                    is_final,
                });
            }
        }
    }

    pub(crate) fn adjust(
        self,
        step: Self,
        increment: bool,
    ) -> Result<Self, NumberInputAdjustError> {
        match self {
            NumberInputValue::F32(value) => Ok(NumberInputValue::F32(if increment {
                value + step.as_f32()
            } else {
                value - step.as_f32()
            })),
            NumberInputValue::F64(value) => Ok(NumberInputValue::F64(if increment {
                value + step.as_f64()
            } else {
                value - step.as_f64()
            })),
            NumberInputValue::I32(value) => {
                let step = step.as_i32()?;
                let next = if increment {
                    value.checked_add(step)
                } else {
                    value.checked_sub(step)
                };
                next.map(NumberInputValue::I32)
                    .ok_or(NumberInputAdjustError::Overflow)
            }
            NumberInputValue::I64(value) => {
                let step = step.as_i64()?;
                let next = if increment {
                    value.checked_add(step)
                } else {
                    value.checked_sub(step)
                };
                next.map(NumberInputValue::I64)
                    .ok_or(NumberInputAdjustError::Overflow)
            }
        }
    }

    fn as_f32(self) -> f32 {
        match self {
            NumberInputValue::F32(value) => value,
            NumberInputValue::F64(value) => value as f32,
            NumberInputValue::I32(value) => value as f32,
            NumberInputValue::I64(value) => value as f32,
        }
    }

    fn as_f64(self) -> f64 {
        match self {
            NumberInputValue::F32(value) => value.into(),
            NumberInputValue::F64(value) => value,
            NumberInputValue::I32(value) => value.into(),
            NumberInputValue::I64(value) => value as f64,
        }
    }

    fn as_i32(self) -> Result<i32, NumberInputAdjustError> {
        match self {
            NumberInputValue::I32(value) => Ok(value),
            NumberInputValue::I64(value) => {
                i32::try_from(value).map_err(|_| NumberInputAdjustError::StepOutOfRangeForI32)
            }
            NumberInputValue::F32(_) | NumberInputValue::F64(_) => {
                Err(NumberInputAdjustError::IntegerStepRequired)
            }
        }
    }

    fn as_i64(self) -> Result<i64, NumberInputAdjustError> {
        match self {
            NumberInputValue::I32(value) => Ok(value.into()),
            NumberInputValue::I64(value) => Ok(value),
            NumberInputValue::F32(_) | NumberInputValue::F64(_) => {
                Err(NumberInputAdjustError::IntegerStepRequired)
            }
        }
    }
}

impl core::fmt::Display for NumberInputValue {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            NumberInputValue::F32(value) => write!(f, "{value}"),
            NumberInputValue::F64(value) => write!(f, "{value}"),
            NumberInputValue::I32(value) => write!(f, "{value}"),
            NumberInputValue::I64(value) => write!(f, "{value}"),
        }
    }
}

/// Programmatically replace the displayed value of a [`NumberInput`].
///
/// If the input currently has focus, the update is ignored so it does not interfere with typing.
#[derive(Clone, EntityEvent)]
pub struct SetNumberInputValue {
    /// The target input.
    #[event_target]
    pub entity: Entity,
    /// The value to display.
    pub value: NumberInputValue,
}

// Programmatic text refreshes still trigger `TextEditChange` on the next frame. Mark them so the
// next change event is ignored instead of echoed back as a user-originated `ValueChange`.
#[derive(Component, Default)]
pub(crate) struct IgnoreNextNumberInputChange;

fn number_input_allows_char(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'e' | 'E')
}

fn number_input_on_insert(insert: On<Insert, NumberInput>, mut world: DeferredWorld) {
    if !world.entity(insert.entity).contains::<EditableTextFilter>() {
        world
            .commands()
            .entity(insert.entity)
            .insert(EditableTextFilter::new(number_input_allows_char));
    }
}

fn number_input_on_text_change(
    change: On<TextEditChange>,
    q_number_input: Query<
        (
            &NumberInput,
            &EditableText,
            Has<IgnoreNextNumberInputChange>,
        ),
        With<NumberInput>,
    >,
    mut commands: Commands,
) {
    let Ok((number_input, editable_text, suppress)) = q_number_input.get(change.event_target())
    else {
        return;
    };

    if suppress {
        commands
            .entity(change.event_target())
            .remove::<IgnoreNextNumberInputChange>();
        return;
    }

    let text_value = editable_text.value().to_string();
    emit_value_change(
        &text_value,
        number_input.format,
        change.event_target(),
        false,
        &mut commands,
    );
}

// Single-line numeric inputs do not lose focus on Enter, so treat Enter as an explicit commit.
fn number_input_on_submit_key(
    key_input: On<FocusedInput<KeyboardInput>>,
    q_number_input: Query<(&NumberInput, &EditableText)>,
    mut commands: Commands,
) {
    if key_input.input.key_code != KeyCode::Enter
        || key_input.input.state != ButtonState::Pressed
        || key_input.input.repeat
    {
        return;
    }

    let Ok((number_input, editable_text)) = q_number_input.get(key_input.focused_entity) else {
        return;
    };

    let text_value = editable_text.value().to_string();
    emit_value_change(
        &text_value,
        number_input.format,
        key_input.focused_entity,
        true,
        &mut commands,
    );
}

fn number_input_on_focus_loss(
    focus_lost: On<FocusLost>,
    q_number_input: Query<(&NumberInput, &EditableText)>,
    mut commands: Commands,
) {
    let Ok((number_input, editable_text)) = q_number_input.get(focus_lost.event_target()) else {
        return;
    };

    let text_value = editable_text.value().to_string();
    emit_value_change(
        &text_value,
        number_input.format,
        focus_lost.event_target(),
        true,
        &mut commands,
    );
}

fn number_input_on_set_value(
    set_value: On<SetNumberInputValue>,
    mut q_number_input: Query<&mut EditableText, With<NumberInput>>,
    focus: Option<Res<InputFocus>>,
    mut commands: Commands,
) {
    queue_number_input_value_update_if_unfocused(
        set_value.event_target(),
        set_value.value,
        &mut q_number_input,
        focus.as_deref(),
        &mut commands,
    );
}

fn emit_value_change(
    text_value: &str,
    format: NumberFormat,
    source: Entity,
    is_final: bool,
    commands: &mut Commands,
) {
    match format.parse(text_value) {
        Ok(value) => value.emit(source, is_final, commands),
        Err(NumberInputParseError::Empty) => {}
        Err(error) => warn!("{error} in text edit"),
    }
}

pub(crate) fn queue_number_input_value_update(
    entity: Entity,
    value: NumberInputValue,
    editable_text: &mut EditableText,
    commands: &mut Commands,
) -> bool {
    let new_digits = value.to_string();
    let old_digits = editable_text.value().to_string();
    if old_digits == new_digits {
        return false;
    }

    commands.entity(entity).insert(IgnoreNextNumberInputChange);
    editable_text.queue_edit(TextEdit::SelectAll);
    editable_text.queue_edit(TextEdit::Insert(new_digits.into()));
    true
}

pub(crate) fn queue_number_input_value_update_if_unfocused(
    entity: Entity,
    value: NumberInputValue,
    q_number_input: &mut Query<&mut EditableText, With<NumberInput>>,
    focus: Option<&InputFocus>,
    commands: &mut Commands,
) -> bool {
    if focus.is_some_and(|focus| focus.get() == Some(entity)) {
        return false;
    }

    let Ok(mut editable_text) = q_number_input.get_mut(entity) else {
        return false;
    };

    queue_number_input_value_update(entity, value, &mut editable_text, commands)
}

/// Plugin that adds observers for [`NumberInput`].
pub struct NumberInputPlugin;

impl Plugin for NumberInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(number_input_on_insert)
            .add_observer(number_input_on_text_change)
            .add_observer(number_input_on_submit_key)
            .add_observer(number_input_on_focus_loss)
            .add_observer(number_input_on_set_value);
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
    use bevy_input_focus::{FocusLost, InputDispatchPlugin, InputFocus, InputFocusPlugin};
    use bevy_window::{PrimaryWindow, Window};

    #[derive(Resource, Default)]
    struct NumberInputEvents(Vec<(Entity, f32, bool)>);

    fn keyboard_input(key_code: KeyCode) -> KeyboardInput {
        KeyboardInput {
            key_code,
            logical_key: match key_code {
                KeyCode::Enter => Key::Enter,
                _ => unreachable!(),
            },
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }

    #[test]
    fn number_input_parses_values() {
        assert_eq!(
            NumberFormat::F32.parse("1.5"),
            Ok(NumberInputValue::F32(1.5))
        );
        assert_eq!(NumberFormat::I32.parse("-7"), Ok(NumberInputValue::I32(-7)));
        assert_eq!(
            NumberFormat::I32.parse("1.5"),
            Err(NumberInputParseError::InvalidInteger)
        );
        assert_eq!(
            NumberFormat::I32.parse(" "),
            Err(NumberInputParseError::Empty)
        );
    }

    #[test]
    fn focus_loss_emits_final_value_change() {
        let mut app = App::new();
        app.init_resource::<NumberInputEvents>()
            .add_plugins(NumberInputPlugin)
            .add_observer(
                |change: On<ValueChange<f32>>, mut events: ResMut<NumberInputEvents>| {
                    events
                        .0
                        .push((change.source, change.value, change.is_final));
                },
            );

        let entity = app
            .world_mut()
            .spawn((NumberInput::default(), EditableText::new("12.5")))
            .id();

        app.world_mut().commands().trigger(FocusLost { entity });
        app.update();

        assert_eq!(
            app.world().resource::<NumberInputEvents>().0,
            vec![(entity, 12.5, true)]
        );
    }

    #[test]
    fn enter_key_emits_final_value_change() {
        let mut app = App::new();
        app.init_resource::<NumberInputEvents>()
            .add_plugins((
                InputPlugin,
                InputFocusPlugin,
                InputDispatchPlugin,
                NumberInputPlugin,
            ))
            .add_observer(
                |change: On<ValueChange<f32>>, mut events: ResMut<NumberInputEvents>| {
                    events
                        .0
                        .push((change.source, change.value, change.is_final));
                },
            );
        app.world_mut().spawn((Window::default(), PrimaryWindow));
        app.update();

        let entity = app
            .world_mut()
            .spawn((NumberInput::default(), EditableText::new("7.25")))
            .id();
        app.world_mut()
            .insert_resource(InputFocus::from_entity(entity));

        app.world_mut()
            .write_message(keyboard_input(KeyCode::Enter))
            .unwrap();
        app.update();

        assert_eq!(
            app.world().resource::<NumberInputEvents>().0,
            vec![(entity, 7.25, true)]
        );
    }

    #[test]
    fn invalid_focus_loss_does_not_emit_value_change() {
        let mut app = App::new();
        app.init_resource::<NumberInputEvents>()
            .add_plugins(NumberInputPlugin)
            .add_observer(
                |change: On<ValueChange<f32>>, mut events: ResMut<NumberInputEvents>| {
                    events
                        .0
                        .push((change.source, change.value, change.is_final));
                },
            );

        let entity = app
            .world_mut()
            .spawn((NumberInput::default(), EditableText::new("abc")))
            .id();

        app.world_mut().commands().trigger(FocusLost { entity });
        app.update();

        assert!(app.world().resource::<NumberInputEvents>().0.is_empty());
    }

    #[test]
    fn set_number_input_value_marks_unfocused_input_for_refresh() {
        let mut app = App::new();
        app.add_plugins(NumberInputPlugin);

        let entity = app
            .world_mut()
            .spawn((
                NumberInput {
                    format: NumberFormat::I32,
                },
                EditableText::new("4"),
            ))
            .id();

        app.world_mut().commands().trigger(SetNumberInputValue {
            entity,
            value: NumberInputValue::I32(9),
        });
        app.update();

        assert!(app
            .world()
            .entity(entity)
            .contains::<IgnoreNextNumberInputChange>());
    }

    #[test]
    fn set_number_input_value_ignores_focused_input() {
        let mut app = App::new();
        app.add_plugins(NumberInputPlugin);

        let entity = app
            .world_mut()
            .spawn((
                NumberInput {
                    format: NumberFormat::I32,
                },
                EditableText::new("4"),
            ))
            .id();
        app.world_mut()
            .insert_resource(InputFocus::from_entity(entity));

        app.world_mut().commands().trigger(SetNumberInputValue {
            entity,
            value: NumberInputValue::I32(9),
        });
        app.update();

        assert!(!app
            .world()
            .entity(entity)
            .contains::<IgnoreNextNumberInputChange>());
    }
}
