import type { OrderProcessorTools } from "../output/t.agent.types";

const students = [
    { id: 1, name: "Ama Johnson", location: "New York", grade: "A" },
    { id: 2, name: "Kwame Mensah", location: "London", grade: "B+" },
    { id: 3, name: "Yaa Asante", location: "Accra", grade: "A-" },
    { id: 4, name: "Kofi Owusu", location: "Toronto", grade: "B" },
    { id: 5, name: "Akua Boateng", location: "Paris", grade: "A+" },
];

export const tools: OrderProcessorTools = {
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
