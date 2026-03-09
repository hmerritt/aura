export interface ICountStore {
	current: number | string;
}

export const countStore: ICountStore = {
	current: 0
};

export default countStore;
