import { type SxProp } from "lib/type-assertions";

export type LoaderVariantProps = Omit<
	React.JSX.IntrinsicElements["div"],
	"style"
> &
	SxProp & {
		size: number | string;
		durationMs: number;
		color: string;
	};
