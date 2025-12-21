import { Agent } from "./loader/IrInterpreter";
import { GoogleDriver } from "./loader/drivers/GoogleDriver";
import data from "../output/t.agent.json"
import type { OrderProcessorInput, OrderProcessorOutput, OrderProcessorTools } from "../output/t.agent.types"
import { OpenAIDriver } from "./loader/drivers/OpenAIDriver";
import { kimi, TempKey } from "./keys";



const driver = new GoogleDriver(TempKey);

const driver1 = new OpenAIDriver(kimi, "https://api.moonshot.ai/v1")

const agent = new Agent<OrderProcessorInput, OrderProcessorOutput>(driver1);
agent.load(data as any);

const students = [
    { id: 1, name: "Ama Johnson", location: "New York", grade: "A" },
    { id: 2, name: "Kwame Mensah", location: "London", grade: "B+" },
    { id: 3, name: "Yaa Asante", location: "Accra", grade: "A-" },
    { id: 4, name: "Kofi Owusu", location: "Toronto", grade: "B" },
    { id: 5, name: "Akua Boateng", location: "Paris", grade: "A+" },
];

const tools: OrderProcessorTools = {
    // Returns the total number of students
    totalstudent: async () => {
        console.log("[Tool] totalstudent called");
        return students.length;
    },

    // Returns the name of a student by ID
    getstudentname: async ({ id }) => {
        console.log(`[Tool] getstudentname called with id: ${id}`);
        const student = students.find(s => s.id === id);
        if (!student) {
            throw new Error(`Student with id ${id} not found`);
        }
        return student.name;
    },

    // Returns the location of a student by ID
    getstudentlocation: async ({ id }) => {
        console.log(`[Tool] getstudentlocation called with id: ${id}`);
        const student = students.find(s => s.id === id);
        if (!student) {
            throw new Error(`Student with id ${id} not found`);
        }
        return student.location;
    },

    // Returns the grade of a student by ID
    getstudentgrade: async ({ id }) => {
        console.log(`[Tool] getstudentgrade called with id: ${id}`);
        const student = students.find(s => s.id === id);
        if (!student) {
            throw new Error(`Student with id ${id} not found`);
        }
        return student.grade;
    },
};


const result = await agent.run({
    request: "what is the details of student with an id 10"
}, tools);

console.log("final", result);







