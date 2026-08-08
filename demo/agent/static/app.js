const task = document.querySelector("#task");
const input = document.querySelector("#input");
const live = document.querySelector("#live");
const run = document.querySelector("#run");
let scenario = "json_analysis";

document.querySelectorAll(".scenario").forEach((button) => {
  button.addEventListener("click", () => {
    document.querySelectorAll(".scenario").forEach((item) => item.classList.remove("active"));
    button.classList.add("active");
    scenario = button.dataset.value;
    if (scenario === "rule_processing") {
      task.value = "校验订单金额，并按会员等级计算每条订单的应付金额。";
      input.value = JSON.stringify({orders:[{id:"R01",member:"gold",amount:1000},{id:"R02",member:"silver",amount:480},{id:"R03",member:"normal",amount:-2}]}, null, 2);
    } else {
      task.value = "统计每个地区的销售额，返回销售额最高的三个地区。";
      input.value = JSON.stringify({orders:[{id:"A01",region:"华东",amount:1280},{id:"A02",region:"华南",amount:860},{id:"A03",region:"华东",amount:720},{id:"A04",region:"华北",amount:990},{id:"A05",region:"华南",amount:1480}]}, null, 2);
    }
  });
});

run.addEventListener("click", async () => {
  const error = document.querySelector("#error");
  error.hidden = true;
  let parsedInput;
  try { parsedInput = JSON.parse(input.value); }
  catch (_) { showError("输入数据不是有效 JSON"); return; }
  run.disabled = true;
  run.innerHTML = "执行中…";
  try {
    const response = await fetch("/api/agent", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({task: task.value, input: parsedInput, scenario, mode: live.checked ? "deepseek" : "offline"})
    });
    const data = await response.json();
    if (!response.ok || !data.ok) throw new Error(data.error?.message || "Agent 执行失败");
    document.querySelector("#empty").hidden = true;
    document.querySelector("#output").hidden = false;
    document.querySelector("#model").textContent = data.model;
    document.querySelector("#timing").textContent = `Model ${data.metrics.modelMs} ms · AgentJS ${data.metrics.agentjsMs} ms`;
    document.querySelector("#plan").textContent = data.plan;
    document.querySelector("#code").textContent = data.code;
    document.querySelector("#result").textContent = JSON.stringify(data.result, null, 2);
  } catch (reason) { showError(reason.message); }
  finally { run.disabled = false; run.innerHTML = "运行 Agent <span>→</span>"; }
});

function showError(message) {
  const error = document.querySelector("#error");
  error.textContent = message;
  error.hidden = false;
}
