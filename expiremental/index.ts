const url = "https://api.groq.com/openai/v1/chat/completions"

const tempKey = "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"

const messages = [
    {
        role: "user",
        content: [
            {
                type: "text",
                text: "What'\''s in this image?"
            },
            {
                type: "image_url",
                image_url: {
                    url: "https://upload.wikimedia.org/wikipedia/commons/f/f2/LPU-v1-die.jpg"
                }
            }
        ]
    }
]

const method = 'POST'
const headers = {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${tempKey}`
}

const body = JSON.stringify({
    messages: messages,
    stream: false,
    model: "meta-llama/llama-4-scout-17b-16e-instruct",
    temperature: 1,
})

const req = await fetch(url, { method, headers, body })
const response = await req.json()

console.log(response)