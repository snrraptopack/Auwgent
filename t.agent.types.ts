// Auto-generated from OrderProcessor

export interface OrderProcessorInput {
    request: string
}

export interface OrderProcessorOutput {
    result: string
}

export interface OrderProcessorTools {
    [key: string]: (args: any) => Promise<any>;  // Index signature for ToolMap compatibility
    getstudentgrade: (args: { id: number }) => Promise<string>;
    getstudentlocation: (args: { id: number }) => Promise<string>;
    totalstudent: (args: {  }) => Promise<number>;
    getstudentname: (args: { id: number }) => Promise<string>;
    getKwamen: (args: {  }) => Promise<string>;
}
