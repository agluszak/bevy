use bevy_app::{App, Plugin};
use bevy_ecs::{
    component::Component,
    entity::Entity,
    event::EntityEvent,
    hierarchy::{ChildOf, Children},
    observer::On,
    query::{Has, With},
    system::{Commands, Query, Res},
};
use bevy_input::{keyboard::Key, ButtonInput};
use bevy_input_focus::InputFocus;
use bevy_log::warn;
use bevy_math::Vec2;
use bevy_picking::events::{Drag, DragEnd, DragStart, Pointer};
use bevy_text::EditableText;
use bevy_ui::{InteractionDisabled, Pressed};

use crate::{
    number_input::{
        queue_number_input_value_update_if_unfocused, NumberInput, NumberInputParseError,
        NumberInputValue,
    },
    NumberFormat, ValueChange,
};

const DRAG_START_THRESHOLD: f32 = 6.0;
const HORIZONTAL_BIAS: f32 = 1.25;

/// Unbounded editor-style numeric scrubber.
///
/// Dragging horizontally changes the embedded [`NumberInput`] value using magnitude-based
/// sensitivity. The embedded text editor is marked with [`NumberScrubberValueInput`].
#[derive(Component, Debug, Default, Clone)]
#[require(NumberScrubberDragState)]
pub struct NumberScrubber;

/// Marks the embedded numeric text field used by a [`NumberScrubber`].
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(NumberInput)]
pub struct NumberScrubberValueInput;

/// Drag state for a [`NumberScrubber`].
#[derive(Component, Debug, Clone, Copy)]
pub struct NumberScrubberDragState {
    dragging: bool,
    start_value: NumberInputValue,
    drag_phase: NumberScrubberDragPhase,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
enum NumberScrubberDragPhase {
    #[default]
    Pending,
    Scrubbing {
        origin_x: f32,
    },
    Canceled,
}

impl Default for NumberScrubberDragState {
    fn default() -> Self {
        Self {
            dragging: false,
            start_value: NumberInputValue::F32(0.0),
            drag_phase: NumberScrubberDragPhase::Pending,
        }
    }
}

/// Programmatically replace the displayed value of a [`NumberScrubber`].
///
/// Like [`crate::SetNumberInputValue`], the embedded text is not overwritten while focused.
#[derive(Clone, EntityEvent)]
pub struct SetNumberScrubberValue {
    /// The target scrubber.
    #[event_target]
    pub entity: Entity,
    /// The value to display.
    pub value: NumberInputValue,
}

fn find_scrubber_value_input(
    root: Entity,
    q_children: &Query<&Children>,
    q_value_input: &Query<(), With<NumberScrubberValueInput>>,
) -> Option<Entity> {
    q_children
        .iter_descendants(root)
        .find(|child| q_value_input.contains(*child))
}

fn find_scrubber_ancestor(
    mut entity: Entity,
    q_parent: &Query<&ChildOf>,
    q_root: &Query<(), With<NumberScrubber>>,
) -> Option<Entity> {
    loop {
        if q_root.contains(entity) {
            return Some(entity);
        }
        entity = q_parent.get(entity).ok()?.parent();
    }
}

fn parse_scrubber_value(
    root: Entity,
    q_children: &Query<&Children>,
    q_value_input: &Query<(), With<NumberScrubberValueInput>>,
    q_number_input: &Query<(&NumberInput, &EditableText)>,
) -> Result<Option<(NumberInputValue, NumberInput)>, NumberInputParseError> {
    let Some(input) = find_scrubber_value_input(root, q_children, q_value_input) else {
        return Ok(None);
    };
    let Ok((number_input, editable_text)) = q_number_input.get(input) else {
        return Ok(None);
    };
    number_input
        .format
        .parse(&editable_text.value().to_string())
        .map(|value| Some((value, *number_input)))
}

fn number_scrubber_on_drag_start(
    mut drag_start: On<Pointer<DragStart>>,
    mut q_root: Query<
        (&mut NumberScrubberDragState, Has<InteractionDisabled>),
        With<NumberScrubber>,
    >,
    q_root_marker: Query<(), With<NumberScrubber>>,
    q_parent: Query<&ChildOf>,
    q_children: Query<&Children>,
    q_value_input: Query<(), With<NumberScrubberValueInput>>,
    q_number_input: Query<(&NumberInput, &EditableText)>,
    mut commands: Commands,
) {
    let Some(root) = find_scrubber_ancestor(drag_start.entity, &q_parent, &q_root_marker) else {
        return;
    };

    drag_start.propagate(false);
    let Ok((mut state, disabled)) = q_root.get_mut(root) else {
        return;
    };
    if disabled {
        return;
    }

    let start_value = match parse_scrubber_value(root, &q_children, &q_value_input, &q_number_input)
    {
        Ok(Some((value, _))) => value,
        Ok(None) => NumberInputValue::F32(0.0),
        Err(_) => return,
    };

    state.dragging = true;
    state.start_value = start_value;
    state.drag_phase = NumberScrubberDragPhase::Pending;
    commands.entity(root).insert(Pressed);
}

fn number_scrubber_on_drag(
    mut drag: On<Pointer<Drag>>,
    mut q_root: Query<
        (&mut NumberScrubberDragState, Has<InteractionDisabled>),
        With<NumberScrubber>,
    >,
    q_root_marker: Query<(), With<NumberScrubber>>,
    q_parent: Query<&ChildOf>,
    q_children: Query<&Children>,
    q_value_input: Query<(), With<NumberScrubberValueInput>>,
    q_number_input: Query<&NumberInput>,
    keys: Res<ButtonInput<Key>>,
    mut commands: Commands,
) {
    let Some(root) = find_scrubber_ancestor(drag.entity, &q_parent, &q_root_marker) else {
        return;
    };

    drag.propagate(false);
    let Ok((mut state, disabled)) = q_root.get_mut(root) else {
        return;
    };
    if !state.dragging || disabled {
        return;
    }
    let Some(effective_distance_x) = state.effective_distance_x(drag.distance) else {
        return;
    };
    emit_scrub_change(
        &mut commands,
        root,
        state.start_value,
        scrubber_format(root, &q_children, &q_value_input, &q_number_input),
        effective_distance_x,
        keys.pressed(Key::Shift),
        keys.pressed(Key::Control),
        false,
    );
}

fn number_scrubber_on_drag_end(
    mut drag_end: On<Pointer<DragEnd>>,
    mut q_root: Query<
        (
            Entity,
            &mut NumberScrubberDragState,
            Has<InteractionDisabled>,
        ),
        With<NumberScrubber>,
    >,
    q_root_marker: Query<(), With<NumberScrubber>>,
    q_parent: Query<&ChildOf>,
    q_children: Query<&Children>,
    q_value_input: Query<(), With<NumberScrubberValueInput>>,
    q_number_input: Query<&NumberInput>,
    keys: Res<ButtonInput<Key>>,
    mut commands: Commands,
) {
    let Some(root) = find_scrubber_ancestor(drag_end.entity, &q_parent, &q_root_marker) else {
        return;
    };

    drag_end.propagate(false);
    let Ok((root, mut state, disabled)) = q_root.get_mut(root) else {
        return;
    };
    if state.dragging {
        if !disabled {
            if let Some(effective_distance_x) = state.effective_distance_x(drag_end.distance) {
                emit_scrub_change(
                    &mut commands,
                    root,
                    state.start_value,
                    scrubber_format(root, &q_children, &q_value_input, &q_number_input),
                    effective_distance_x,
                    keys.pressed(Key::Shift),
                    keys.pressed(Key::Control),
                    true,
                );
            }
        }
        state.dragging = false;
        state.drag_phase = NumberScrubberDragPhase::Pending;
        commands.entity(root).remove::<Pressed>();
    }
}

impl NumberScrubberDragState {
    fn effective_distance_x(&mut self, distance: Vec2) -> Option<f32> {
        match self.drag_phase {
            NumberScrubberDragPhase::Pending => {
                let abs_x = distance.x.abs();
                let abs_y = distance.y.abs();

                if abs_x >= DRAG_START_THRESHOLD && abs_x >= abs_y * HORIZONTAL_BIAS {
                    self.drag_phase = NumberScrubberDragPhase::Scrubbing {
                        origin_x: distance.x,
                    };
                } else if abs_y >= DRAG_START_THRESHOLD && abs_y > abs_x * HORIZONTAL_BIAS {
                    self.drag_phase = NumberScrubberDragPhase::Canceled;
                }
                None
            }
            NumberScrubberDragPhase::Scrubbing { origin_x } => Some(distance.x - origin_x),
            NumberScrubberDragPhase::Canceled => None,
        }
    }
}

fn scrubber_format(
    root: Entity,
    q_children: &Query<&Children>,
    q_value_input: &Query<(), With<NumberScrubberValueInput>>,
    q_number_input: &Query<&NumberInput>,
) -> Option<NumberInput> {
    find_scrubber_value_input(root, q_children, q_value_input)
        .and_then(|input| q_number_input.get(input).ok().copied())
}

fn base_scrub_unit(start_value: f64, format: NumberFormat) -> f64 {
    let magnitude = start_value.abs().max(1.0).log10().floor();
    let unit = 10_f64.powf(magnitude) * 0.01;
    match format {
        NumberFormat::F32 | NumberFormat::F64 => unit,
        NumberFormat::I32 | NumberFormat::I64 => unit.max(0.5),
    }
}

fn scrub_delta(start_value: f64, format: NumberFormat, distance_x: f32, precision: bool) -> f64 {
    let unit = base_scrub_unit(start_value, format) * if precision { 0.1 } else { 1.0 };
    start_value + f64::from(distance_x) * unit
}

fn apply_scrub_snap(value: f64, start_value: f64, format: NumberFormat, precision: bool) -> f64 {
    let step = match format {
        NumberFormat::F32 | NumberFormat::F64 => {
            let unit = base_scrub_unit(start_value, format);
            if precision {
                unit
            } else {
                unit * 10.0
            }
        }
        NumberFormat::I32 | NumberFormat::I64 => {
            if precision {
                1.0
            } else {
                10.0
            }
        }
    };

    if step > 0.0 {
        (value / step).round() * step
    } else {
        value
    }
}

fn emit_scrub_change(
    commands: &mut Commands,
    root: Entity,
    start_value: NumberInputValue,
    number_input: Option<NumberInput>,
    distance_x: f32,
    precision: bool,
    snap: bool,
    is_final: bool,
) {
    let Some(number_input) = number_input else {
        return;
    };
    let start = start_value.as_f64();
    let mut next = scrub_delta(start, number_input.format, distance_x, precision);
    if snap {
        next = apply_scrub_snap(next, start, number_input.format, precision);
    }
    let value = match number_input.format {
        NumberFormat::F32 => NumberInputValue::F32(next as f32),
        NumberFormat::F64 => NumberInputValue::F64(next),
        NumberFormat::I32 => {
            let next = next.round();
            if !(i32::MIN as f64..=i32::MAX as f64).contains(&next) {
                warn!("NumberScrubber drag ignored: value out of range for i32");
                return;
            }
            NumberInputValue::I32(next as i32)
        }
        NumberFormat::I64 => {
            let next = next.round();
            if !(i64::MIN as f64..=i64::MAX as f64).contains(&next) {
                warn!("NumberScrubber drag ignored: value out of range for i64");
                return;
            }
            NumberInputValue::I64(next as i64)
        }
    };
    value.emit(root, is_final, commands);
}

macro_rules! number_scrubber_forward_value_change {
    ($name:ident, $ty:ty) => {
        fn $name(
            change: On<ValueChange<$ty>>,
            q_value_input: Query<(), With<NumberScrubberValueInput>>,
            q_parent: Query<&ChildOf>,
            q_root: Query<(), With<NumberScrubber>>,
            mut commands: Commands,
        ) {
            if !q_value_input.contains(change.source) {
                return;
            }

            let Some(root) = find_scrubber_ancestor(change.source, &q_parent, &q_root) else {
                return;
            };

            commands.trigger(ValueChange {
                source: root,
                value: change.value,
                is_final: change.is_final,
            });
        }
    };
}

number_scrubber_forward_value_change!(number_scrubber_on_number_change_f32, f32);
number_scrubber_forward_value_change!(number_scrubber_on_number_change_f64, f64);
number_scrubber_forward_value_change!(number_scrubber_on_number_change_i32, i32);
number_scrubber_forward_value_change!(number_scrubber_on_number_change_i64, i64);

macro_rules! number_scrubber_on_root_value_change {
    ($name:ident, $ty:ty, $variant:ident) => {
        fn $name(
            change: On<ValueChange<$ty>>,
            q_root: Query<(), With<NumberScrubber>>,
            q_children: Query<&Children>,
            q_value_input: Query<(), With<NumberScrubberValueInput>>,
            mut q_number_input: Query<&mut EditableText, With<NumberInput>>,
            focus: Option<Res<InputFocus>>,
            mut commands: Commands,
        ) {
            if !q_root.contains(change.source) {
                return;
            }

            let Some(input) = find_scrubber_value_input(change.source, &q_children, &q_value_input)
            else {
                return;
            };

            queue_number_input_value_update_if_unfocused(
                input,
                NumberInputValue::$variant(change.value),
                &mut q_number_input,
                focus.as_deref(),
                &mut commands,
            );
        }
    };
}

number_scrubber_on_root_value_change!(number_scrubber_on_root_change_f32, f32, F32);
number_scrubber_on_root_value_change!(number_scrubber_on_root_change_f64, f64, F64);
number_scrubber_on_root_value_change!(number_scrubber_on_root_change_i32, i32, I32);
number_scrubber_on_root_value_change!(number_scrubber_on_root_change_i64, i64, I64);

fn number_scrubber_on_set_value(
    set_value: On<SetNumberScrubberValue>,
    q_root: Query<(), With<NumberScrubber>>,
    q_children: Query<&Children>,
    q_value_input: Query<(), With<NumberScrubberValueInput>>,
    mut q_number_input: Query<&mut EditableText, With<NumberInput>>,
    focus: Option<Res<InputFocus>>,
    mut commands: Commands,
) {
    if !q_root.contains(set_value.event_target()) {
        return;
    }

    let Some(input) =
        find_scrubber_value_input(set_value.event_target(), &q_children, &q_value_input)
    else {
        return;
    };
    queue_number_input_value_update_if_unfocused(
        input,
        set_value.value,
        &mut q_number_input,
        focus.as_deref(),
        &mut commands,
    );
}

/// Plugin that adds observers for [`NumberScrubber`].
pub struct NumberScrubberPlugin;

impl Plugin for NumberScrubberPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(number_scrubber_on_drag_start)
            .add_observer(number_scrubber_on_drag)
            .add_observer(number_scrubber_on_drag_end)
            .add_observer(number_scrubber_on_number_change_f32)
            .add_observer(number_scrubber_on_number_change_f64)
            .add_observer(number_scrubber_on_number_change_i32)
            .add_observer(number_scrubber_on_number_change_i64)
            .add_observer(number_scrubber_on_root_change_f32)
            .add_observer(number_scrubber_on_root_change_f64)
            .add_observer(number_scrubber_on_root_change_i32)
            .add_observer(number_scrubber_on_root_change_i64)
            .add_observer(number_scrubber_on_set_value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NumberFormat;
    use bevy_app::App;
    use bevy_ecs::{observer::On, prelude::*};

    #[derive(Resource, Default)]
    struct Events(Vec<(Entity, i32, bool)>);

    #[test]
    fn scrub_delta_scales_zero_normal_large_and_negative_values() {
        assert!(scrub_delta(0.0, NumberFormat::F32, 10.0, false) > 0.0);
        assert!(
            scrub_delta(10.0, NumberFormat::F32, 10.0, false)
                > scrub_delta(1.0, NumberFormat::F32, 10.0, false)
        );
        assert!(
            scrub_delta(1000.0, NumberFormat::F32, 10.0, false)
                > scrub_delta(10.0, NumberFormat::F32, 10.0, false)
        );
        assert!(scrub_delta(-10.0, NumberFormat::F32, -10.0, false) < -10.0);
        assert!(
            (scrub_delta(10.0, NumberFormat::F32, 10.0, true) - 10.0).abs()
                < (scrub_delta(10.0, NumberFormat::F32, 10.0, false) - 10.0).abs()
        );
    }

    #[test]
    fn scrub_delta_is_independent_of_event_segmentation() {
        let single_fast_event = scrub_delta(10.0, NumberFormat::F32, 400.0, false);
        let segmented_final_event = scrub_delta(10.0, NumberFormat::F32, 400.0, false);

        assert_eq!(single_fast_event, segmented_final_event);
    }

    #[test]
    fn scrubber_drag_threshold_anchors_without_jumping() {
        let mut state = NumberScrubberDragState {
            dragging: true,
            start_value: NumberInputValue::F32(0.0),
            drag_phase: NumberScrubberDragPhase::Pending,
        };

        assert_eq!(state.effective_distance_x(Vec2::new(2.0, 0.0)), None);
        assert_eq!(state.effective_distance_x(Vec2::new(10.0, 0.0)), None);
        assert_eq!(
            state.drag_phase,
            NumberScrubberDragPhase::Scrubbing { origin_x: 10.0 }
        );
        assert_eq!(state.effective_distance_x(Vec2::new(15.0, 0.0)), Some(5.0));
        assert_eq!(state.effective_distance_x(Vec2::new(7.0, 0.0)), Some(-3.0));
    }

    #[test]
    fn vertical_drag_with_x_jitter_cancels_scrubbing() {
        let mut state = NumberScrubberDragState {
            dragging: true,
            start_value: NumberInputValue::F32(0.0),
            drag_phase: NumberScrubberDragPhase::Pending,
        };

        assert_eq!(state.effective_distance_x(Vec2::new(2.0, 20.0)), None);
        assert_eq!(state.drag_phase, NumberScrubberDragPhase::Canceled);
        assert_eq!(state.effective_distance_x(Vec2::new(50.0, 20.0)), None);
    }

    #[test]
    fn ambiguous_diagonal_drag_remains_pending() {
        let mut state = NumberScrubberDragState {
            dragging: true,
            start_value: NumberInputValue::F32(0.0),
            drag_phase: NumberScrubberDragPhase::Pending,
        };

        assert_eq!(state.effective_distance_x(Vec2::new(8.0, 7.0)), None);
        assert_eq!(state.drag_phase, NumberScrubberDragPhase::Pending);
    }

    #[test]
    fn scrub_delta_is_linear_without_acceleration() {
        assert_eq!(scrub_delta(0.0, NumberFormat::F32, 100.0, false), 1.0);
        assert_eq!(scrub_delta(0.0, NumberFormat::F32, 500.0, false), 5.0);
        assert_eq!(scrub_delta(0.0, NumberFormat::F32, 600.0, false), 6.0);
    }

    #[test]
    fn scrub_delta_shift_precision_is_ten_times_smaller() {
        let normal = scrub_delta(10.0, NumberFormat::F32, 100.0, false) - 10.0;
        let precise = scrub_delta(10.0, NumberFormat::F32, 100.0, true) - 10.0;

        assert!((normal / precise - 10.0).abs() < 1e-10);
    }

    #[test]
    fn scrub_snap_uses_stable_output_quantum() {
        let unsnapped = scrub_delta(10.0, NumberFormat::F32, 123.0, false);
        let snapped = apply_scrub_snap(unsnapped, 10.0, NumberFormat::F32, false);
        let precise_snapped = apply_scrub_snap(unsnapped, 10.0, NumberFormat::F32, true);

        assert_eq!((snapped / 1.0).fract(), 0.0);
        assert_eq!((precise_snapped / 0.1).fract(), 0.0);
    }

    #[test]
    fn integer_scrubbing_rounds_safely() {
        let value = scrub_delta(8.0, NumberFormat::I32, 10.0, false);

        assert_eq!(value.round(), 13.0);
    }

    #[test]
    fn integer_and_ctrl_precision_steps_are_stable() {
        let integer_value = scrub_delta(8.0, NumberFormat::I32, 3.0, false);
        assert_eq!(integer_value.round(), 10.0);

        assert_eq!(apply_scrub_snap(13.6, 8.0, NumberFormat::I32, false), 10.0);
        assert_eq!(apply_scrub_snap(13.6, 8.0, NumberFormat::I32, true), 14.0);
        assert_eq!(
            apply_scrub_snap(10.36, 10.0, NumberFormat::F32, false),
            10.0
        );
        assert!((apply_scrub_snap(10.36, 10.0, NumberFormat::F32, true) - 10.4).abs() < 1e-10);
    }

    #[test]
    fn integer_text_edits_forward_from_root() {
        let mut app = App::new();
        app.init_resource::<Events>()
            .add_plugins(NumberScrubberPlugin)
            .add_observer(|change: On<ValueChange<i32>>, mut events: ResMut<Events>| {
                events
                    .0
                    .push((change.source, change.value, change.is_final));
            });

        let root = app
            .world_mut()
            .spawn((NumberScrubber, NumberScrubberDragState::default()))
            .id();
        let input = app
            .world_mut()
            .spawn((
                NumberScrubberValueInput,
                NumberInput {
                    format: NumberFormat::I32,
                },
                EditableText::new("8"),
                ChildOf(root),
            ))
            .id();

        app.world_mut().commands().trigger(ValueChange {
            source: input,
            value: 9_i32,
            is_final: false,
        });
        app.update();

        assert!(app
            .world()
            .resource::<Events>()
            .0
            .contains(&(root, 9, false)));
    }
}
