const colors = [
	"#d5f1ff",
	"#cdfff5",
	"#cdffda",
	"#fcffcd",
	"#ffeecd",
	"#d5e0ff",
	"#97b9ed",
	"#a3a2ff",
	"#FFFFFF",
	"#FFE799",
	"#F8B47C"
];

export interface IColorStore {
	colors: string[];
	current: string;
}

export const colorStore: IColorStore = {
	colors,
	current: colors[0]
};

export default colorStore;
