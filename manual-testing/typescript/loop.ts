
// Start an interactive CLI loop — type messages to send to the agent.
const readline = await import("readline")

export function startRepl(agent: any) {
    const rl = readline.createInterface({ input: process.stdin, output: process.stdout, prompt: "> " })

    rl.on("line", async (line: string) => {
        const trimmed = line.trim()
        if (!trimmed) {
            rl.prompt()
            return
        }
        if (trimmed === "exit" || trimmed === "quit") {
            rl.close()
            return
        }

        try {
          let session = await agent.run(trimmed)
          //console.log("This log is from the repl loop file")
           // console.log(JSON.stringify(session.turns, null, 2))
        } catch (err) {
            console.error("Agent error:", err)
        }

        rl.prompt()
    })

    rl.on("close", () => {
        console.log("Goodbye.")
        process.exit(0)
    })

    rl.prompt()
}
