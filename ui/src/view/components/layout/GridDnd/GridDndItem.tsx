import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

import { GridDndDragBox } from "./GridDndDragBox";

export type GridDndItemProps = {
	id: string;
	dataItem: any;
	renderIndex: number;
	renderWith: (props: any) => React.JSX.Element;
};

export const GridDndItem = ({
	id,
	dataItem,
	renderIndex,
	renderWith
}: GridDndItemProps) => {
	const RenderWith = renderWith;

	const { active, attributes, listeners, setNodeRef, transform, transition } =
		useSortable({ id: id });

	const style = {
		zIndex: active?.id === id ? 50 : 1,
		transform: CSS.Transform.toString(transform),
		transition
	};

	return (
		<div ref={setNodeRef} style={style} {...attributes} {...listeners}>
			{active?.id === id ? (
				<GridDndDragBox
					dataItem={dataItem}
					renderIndex={renderIndex}
					renderWith={renderWith}
				/>
			) : (
				<RenderWith {...dataItem} renderIndex={renderIndex} />
			)}
		</div>
	);
};
