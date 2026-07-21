console.log("AgentJS runtime demo");

function chooseTool(task) {
    if (task.type === "math") return "calculator";
    if (task.type === "text") return "summarizer";
    if (task.type === "data") return "json_parser";
    return "fallback";
}

let tasks = [
    { id: 1, type: "math", input: "1 + 2 * 3" },
    { id: 2, type: "text", input: "summarize project report" },
    { id: 3, type: "data", input: "{\"score\": 95}" }
];

let plan = [];

for (let i = 0; i < tasks.length; i++) {
    let task = tasks[i];
    plan.push({
        taskId: task.id,
        tool: chooseTool(task),
        status: "ready"
    });
}

JSON.stringify(plan);