import {
	DndContext,
	DragEndEvent,
	DragOverlay,
	MouseSensor,
	MouseSensorOptions,
	PointerSensorOptions,
	TouchSensor,
	UniqueIdentifier,
	closestCenter,
	useSensor,
	useSensors
} from "@dnd-kit/core";
import { SortableContext, arrayMove, rectSortingStrategy } from "@dnd-kit/sortable";
import { useState } from "react";

import { Grid, GridProps } from "view/components";

import { GridDndItem } from "./GridDndItem";

type Data = {
	id: string;
	[key: string]: any;
};

type RenderWithProps = {
	renderIndex: number;
	[key: string]: any;
};

export type GridDndrops = GridProps & {
	data: Data[];
	mouseSensorOptions?: MouseSensorOptions;
	pointerSensorOptions?: PointerSensorOptions;
	renderWith: (props: RenderWithProps) => React.JSX.Element;
	getProps?: (data: Data, index?: number) => Record<string, unknown>;
	setData: React.Dispatch<React.SetStateAction<any[]>>;
	// Custom drag events
	onDragStart?: (event: DragEndEvent) => void;
	onDragEnd?: (event: DragEndEvent) => void;
	onDragCancel?: () => void;
};

const defaultGetProps = (data: Data, _?: number) => data;

/**
 * Grid with drag and drop functionality.
 *
 * Data must have a unique `id` property.
 */
export const GridDnd = ({
	renderWith: RenderWith,
	data,
	setData,
	onDragStart,
	onDragEnd,
	onDragCancel,
	getProps = defaultGetProps,
	mouseSensorOptions = {
		// Require the mouse to move by 10 pixels before activating
		activationConstraint: {
			distance: 10
		}
	},
	pointerSensorOptions = {
		// Press delay of 100ms, with tolerance of 5px of movement
		activationConstraint: {
			delay: 100,
			tolerance: 5
		}
	},
	...gridProps
}: GridDndrops) => {
	const [activeId, setActiveId] = useState<null | UniqueIdentifier>(-1);

	const sensors = useSensors(
		useSensor(MouseSensor, mouseSensorOptions),
		useSensor(TouchSensor, pointerSensorOptions)
	);

	const handleDragStart = (event: DragEndEvent) => {
		if (onDragStart) onDragStart(event);
		setActiveId(event.active.id);
	};

	const handleDragEnd = (event: DragEndEvent) => {
		if (onDragEnd) onDragEnd(event);

		const { active, over } = event;

		if (active?.id !== over?.id) {
			setData((items) => {
				const oldIndex = items.findIndex((x) => x.id === active?.id);
				const newIndex = items.findIndex((x) => x.id === over?.id);

				return arrayMove(items, oldIndex, newIndex);
			});
		}

		setActiveId(null);
	};

	const handleDragCancel = () => {
		if (onDragCancel) onDragCancel();
		setActiveId(null);
	};

	return (
		<DndContext
			collisionDetection={closestCenter}
			onDragCancel={handleDragCancel}
			onDragEnd={handleDragEnd}
			onDragStart={handleDragStart}
			sensors={sensors}
		>
			<SortableContext
				items={data.map(({ id }) => id)}
				strategy={rectSortingStrategy}
			>
				<Grid {...gridProps}>
					{data.map((dataItem, index) => (
						<GridDndItem
							key={dataItem.id}
							id={dataItem.id}
							dataItem={getProps(dataItem, index)}
							renderIndex={index}
							renderWith={RenderWith}
						/>
					))}
				</Grid>
			</SortableContext>

			<DragOverlay adjustScale={false}>
				{activeId ? (
					<RenderWith
						{...getProps(data?.[data?.findIndex((x) => x.id === activeId)])}
						renderIndex={-1}
					/>
				) : null}
			</DragOverlay>
		</DndContext>
	);
};
