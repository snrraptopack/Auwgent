// Auto-generated from OrderProcessor

export interface OrderProcessorInput {
    request: string
}

export interface OrderProcessorOutput {
    result: string
}

export interface OrderProcessorTools {
    [key: string]: (args: any) => Promise<any>;  // Index signature for ToolMap compatibility
    totalstudent: (args: {  }) => Promise<number>;
    getstudentname: (args: { id: number }) => Promise<string>;
    getstudentlocation: (args: { id: number }) => Promise<string>;
    getstudentgrade: (args: { id: number }) => Promise<string>;
}
