// Auto-generated from Hello

export interface HelloInput {
    text: string
}

export interface HelloOutput {

}

export interface HelloTools {
    getLoction: (args: {  }) => Promise<string>;
    getWeather: (args: {  }) => Promise<string>;
}
