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
use bevy_input_focus::InputFocus;
use bevy_picking::events::{Drag, DragEnd, DragStart, Pointer};
use bevy_text::EditableText;
use bevy_ui::{ComputedNode, InteractionDisabled, Pressed, UiScale};

use crate::{
    number_input::{
        queue_number_input_value_update_if_unfocused, NumberFormat, NumberInput,
        NumberInputParseError, NumberInputValue,
    },
    Slider, SliderDragState, SliderPrecision, SliderRange, SliderValue, ValueChange,
};

/// Bounded editor-style numeric input.
///
/// This is a composite root for a whole-field draggable numeric value. The embedded text editor is
/// marked with [`RangedNumberInputValueInput`]. Dragging emits root-level [`ValueChange<f32>`]
/// events; text edits from the embedded [`NumberInput`] are clamped and forwarded from the root.
#[derive(Component, Debug, Default, Clone)]
#[require(Slider)]
pub struct RangedNumberInput;

/// Marks the embedded numeric text field used by a [`RangedNumberInput`].
#[derive(Component, Debug, Default, Clone, Copy)]
#[require(NumberInput)]
pub struct RangedNumberInputValueInput;

/// Programmatically replace the displayed value of a [`RangedNumberInput`].
///
/// Like [`crate::SetNumberInputValue`], the embedded text is not overwritten while focused.
#[derive(Clone, EntityEvent)]
pub struct SetRangedNumberInputValue {
    /// The target ranged number input.
    #[event_target]
    pub entity: Entity,
    /// The value to display. It is clamped to the input range.
    pub value: f32,
}

fn find_ranged_input_value_input(
    root: Entity,
    q_children: &Query<&Children>,
    q_value_input: &Query<(), With<RangedNumberInputValueInput>>,
) -> Option<Entity> {
    q_children
        .iter_descendants(root)
        .find(|child| q_value_input.contains(*child))
}

fn find_ranged_input_ancestor(
    mut entity: Entity,
    q_parent: &Query<&ChildOf>,
    q_root: &Query<(), With<RangedNumberInput>>,
) -> Option<Entity> {
    loop {
        if q_root.contains(entity) {
            return Some(entity);
        }
        entity = q_parent.get(entity).ok()?.parent();
    }
}

fn parse_embedded_f32(
    root: Entity,
    q_children: &Query<&Children>,
    q_value_input: &Query<(), With<RangedNumberInputValueInput>>,
    q_number_input: &Query<(&NumberInput, &EditableText)>,
) -> Result<Option<f32>, NumberInputParseError> {
    let Some(input) = find_ranged_input_value_input(root, q_children, q_value_input) else {
        return Ok(None);
    };
    let Ok((number_input, editable_text)) = q_number_input.get(input) else {
        return Ok(None);
    };
    number_input
        .format
        .parse(&editable_text.value().to_string())
        .map(|value| Some(value.as_f32()))
}

fn ranged_number_input_on_drag_start(
    mut drag_start: On<Pointer<DragStart>>,
    mut q_root: Query<
        (&SliderValue, &mut SliderDragState, Has<InteractionDisabled>),
        With<RangedNumberInput>,
    >,
    q_root_marker: Query<(), With<RangedNumberInput>>,
    q_parent: Query<&ChildOf>,
    q_children: Query<&Children>,
    q_value_input: Query<(), With<RangedNumberInputValueInput>>,
    q_number_input: Query<(&NumberInput, &EditableText)>,
    mut commands: Commands,
) {
    let Some(root) = find_ranged_input_ancestor(drag_start.entity, &q_parent, &q_root_marker)
    else {
        return;
    };

    drag_start.propagate(false);
    let Ok((value, mut drag, disabled)) = q_root.get_mut(root) else {
        return;
    };
    if disabled {
        return;
    }

    let start_value = match parse_embedded_f32(root, &q_children, &q_value_input, &q_number_input) {
        Ok(Some(value)) => value,
        Ok(None) => value.0,
        Err(_) => return,
    };

    drag.dragging = true;
    drag.offset = start_value;
    commands.entity(root).insert(Pressed);
}

fn ranged_number_input_on_drag(
    mut drag: On<Pointer<Drag>>,
    q_root: Query<
        (
            &ComputedNode,
            &SliderRange,
            Option<&SliderPrecision>,
            &SliderDragState,
            Has<InteractionDisabled>,
        ),
        With<RangedNumberInput>,
    >,
    q_root_marker: Query<(), With<RangedNumberInput>>,
    q_parent: Query<&ChildOf>,
    mut commands: Commands,
    ui_scale: Res<UiScale>,
) {
    let Some(root) = find_ranged_input_ancestor(drag.entity, &q_parent, &q_root_marker) else {
        return;
    };

    drag.propagate(false);
    let Ok((node, range, precision, drag_state, disabled)) = q_root.get(root) else {
        return;
    };
    if !drag_state.dragging || disabled {
        return;
    }

    emit_ranged_drag_change(
        &mut commands,
        root,
        node,
        range,
        precision,
        drag_state,
        drag.distance.x / ui_scale.0,
        false,
    );
}

fn ranged_number_input_on_drag_end(
    mut drag_end: On<Pointer<DragEnd>>,
    mut q_root: Query<
        (
            Entity,
            &ComputedNode,
            &SliderRange,
            Option<&SliderPrecision>,
            &mut SliderDragState,
            Has<InteractionDisabled>,
        ),
        With<RangedNumberInput>,
    >,
    q_root_marker: Query<(), With<RangedNumberInput>>,
    q_parent: Query<&ChildOf>,
    mut commands: Commands,
    ui_scale: Res<UiScale>,
) {
    let Some(root) = find_ranged_input_ancestor(drag_end.entity, &q_parent, &q_root_marker) else {
        return;
    };

    drag_end.propagate(false);
    let Ok((root, node, range, precision, mut drag_state, disabled)) = q_root.get_mut(root) else {
        return;
    };
    if drag_state.dragging {
        if !disabled {
            emit_ranged_drag_change(
                &mut commands,
                root,
                node,
                range,
                precision,
                &drag_state,
                drag_end.distance.x / ui_scale.0,
                true,
            );
        }
        drag_state.dragging = false;
        commands.entity(root).remove::<Pressed>();
    }
}

fn emit_ranged_drag_change(
    commands: &mut Commands,
    root: Entity,
    node: &ComputedNode,
    range: &SliderRange,
    precision: Option<&SliderPrecision>,
    drag: &SliderDragState,
    distance_x: f32,
    is_final: bool,
) {
    let size = (node.size().x * node.inverse_scale_factor).max(1.0);
    let value = if range.span() > 0.0 {
        drag.offset + distance_x * range.span() / size
    } else {
        range.center()
    };
    let value = precision.map_or(value, |precision| precision.round(value));
    commands.trigger(ValueChange {
        source: root,
        value: range.clamp(value),
        is_final,
    });
}

fn ranged_number_input_on_number_change(
    change: On<ValueChange<f32>>,
    q_value_input: Query<(), With<RangedNumberInputValueInput>>,
    q_parent: Query<&ChildOf>,
    q_root_marker: Query<(), With<RangedNumberInput>>,
    q_root: Query<&SliderRange>,
    mut commands: Commands,
) {
    if !q_value_input.contains(change.source) {
        return;
    }

    let Some(root) = find_ranged_input_ancestor(change.source, &q_parent, &q_root_marker) else {
        return;
    };
    let Ok(range) = q_root.get(root) else {
        return;
    };

    commands.trigger(ValueChange {
        source: root,
        value: range.clamp(change.value),
        is_final: change.is_final,
    });
}

fn ranged_number_input_on_root_change(
    change: On<ValueChange<f32>>,
    q_root: Query<&SliderRange, With<RangedNumberInput>>,
    q_children: Query<&Children>,
    q_value_input: Query<(), With<RangedNumberInputValueInput>>,
    mut q_number_input: Query<&mut EditableText, With<NumberInput>>,
    focus: Option<Res<InputFocus>>,
    mut commands: Commands,
) {
    let Ok(range) = q_root.get(change.source) else {
        return;
    };
    let value = range.clamp(change.value);
    commands.entity(change.source).insert(SliderValue(value));

    let Some(input) = find_ranged_input_value_input(change.source, &q_children, &q_value_input)
    else {
        return;
    };
    queue_number_input_value_update_if_unfocused(
        input,
        NumberInputValue::F32(value),
        &mut q_number_input,
        focus.as_deref(),
        &mut commands,
    );
}

fn ranged_number_input_on_set_value(
    set_value: On<SetRangedNumberInputValue>,
    q_root: Query<&SliderRange, With<RangedNumberInput>>,
    q_children: Query<&Children>,
    q_value_input: Query<(), With<RangedNumberInputValueInput>>,
    mut q_number_input: Query<&mut EditableText, With<NumberInput>>,
    focus: Option<Res<InputFocus>>,
    mut commands: Commands,
) {
    let Ok(range) = q_root.get(set_value.event_target()) else {
        return;
    };
    let value = range.clamp(set_value.value);
    commands
        .entity(set_value.event_target())
        .insert(SliderValue(value));

    let Some(input) =
        find_ranged_input_value_input(set_value.event_target(), &q_children, &q_value_input)
    else {
        return;
    };
    queue_number_input_value_update_if_unfocused(
        input,
        NumberInputValue::F32(value),
        &mut q_number_input,
        focus.as_deref(),
        &mut commands,
    );
}

fn ranged_number_input_on_insert_value_input(
    insert: On<bevy_ecs::lifecycle::Insert, RangedNumberInputValueInput>,
    mut q_input: Query<&mut NumberInput, With<RangedNumberInputValueInput>>,
) {
    if let Ok(mut input) = q_input.get_mut(insert.entity) {
        input.format = NumberFormat::F32;
    }
}

/// Plugin that adds observers for [`RangedNumberInput`].
pub struct RangedNumberInputPlugin;

impl Plugin for RangedNumberInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(ranged_number_input_on_insert_value_input)
            .add_observer(ranged_number_input_on_drag_start)
            .add_observer(ranged_number_input_on_drag)
            .add_observer(ranged_number_input_on_drag_end)
            .add_observer(ranged_number_input_on_number_change)
            .add_observer(ranged_number_input_on_root_change)
            .add_observer(ranged_number_input_on_set_value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bevy_ecs::{observer::On, prelude::*};

    #[derive(Resource, Default)]
    struct Events(Vec<(Entity, f32, bool)>);

    #[test]
    fn text_edits_forward_from_root_and_clamp() {
        let mut app = App::new();
        app.init_resource::<Events>()
            .add_plugins(RangedNumberInputPlugin)
            .add_observer(|change: On<ValueChange<f32>>, mut events: ResMut<Events>| {
                events
                    .0
                    .push((change.source, change.value, change.is_final));
            });

        let root = app
            .world_mut()
            .spawn((RangedNumberInput, SliderRange::new(0.0, 1.0)))
            .id();
        let input = app
            .world_mut()
            .spawn((
                RangedNumberInputValueInput,
                NumberInput::default(),
                EditableText::new("2.0"),
                ChildOf(root),
            ))
            .id();

        app.world_mut().commands().trigger(ValueChange {
            source: input,
            value: 2.0_f32,
            is_final: false,
        });
        app.update();

        assert!(app
            .world()
            .resource::<Events>()
            .0
            .contains(&(root, 1.0, false)));
    }

    #[test]
    fn set_value_updates_slider_value_and_text() {
        let mut app = App::new();
        app.add_plugins(RangedNumberInputPlugin);

        let root = app
            .world_mut()
            .spawn((RangedNumberInput, SliderRange::new(0.0, 1.0)))
            .id();
        let input = app
            .world_mut()
            .spawn((
                RangedNumberInputValueInput,
                NumberInput::default(),
                EditableText::new("0.0"),
                ChildOf(root),
            ))
            .id();

        app.world_mut()
            .commands()
            .trigger(SetRangedNumberInputValue {
                entity: root,
                value: 3.0,
            });
        app.update();

        assert_eq!(
            app.world().entity(root).get::<SliderValue>(),
            Some(&SliderValue(1.0))
        );
        assert!(app
            .world()
            .entity(input)
            .contains::<crate::number_input::IgnoreNextNumberInputChange>());
    }
}
