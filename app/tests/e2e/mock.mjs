// Local stand-in for Vast.ai (route + worker + console API) so the whole app flow can be tested without keys.
import http from "node:http";
import fs from "node:fs";

const PORT = Number(process.env.MOCK_PORT || 18777);
const MP4 = fs.readFileSync(process.env.MOCK_MP4);
const log = [];
let routeCalls = 0;
let genBodies = [];

const send = (res, code, obj) => { res.writeHead(code, { "content-type": "application/json" }); res.end(JSON.stringify(obj)); };
const readBody = (req) => new Promise((r) => { let b = ""; req.on("data", (c) => (b += c)); req.on("end", () => r(b)); });

http.createServer(async (req, res) => {
  const url = new URL(req.url, `http://127.0.0.1:${PORT}`);
  const body = req.method === "GET" ? "" : await readBody(req);
  const j = body ? JSON.parse(body) : {};
  const auth = req.headers.authorization;
  log.push({ m: req.method, p: url.pathname, auth: !!auth, body: j });

  if (url.pathname === "/__log") return send(res, 200, { log, genBodies });
  if (url.pathname === "/__reset") { log.length = 0; genBodies = []; routeCalls = 0; return send(res, 200, {}); }
  if (url.pathname === "/file.mp4") { res.writeHead(200, { "content-type": "video/mp4" }); return res.end(MP4); }

  if (url.pathname === "/route/") {
    if (!auth || auth === "Bearer bad") return send(res, 200, { error_msg: "endpoint 0 not found or unauthorized" });
    routeCalls++;
    // cold start: first two polls of every new request have no worker
    if (j.request_idx === 0 || routeCalls % 3 !== 0) return send(res, 200, { endpoint: j.endpoint, status: "Stopped", request_idx: 1000 + routeCalls });
    return send(res, 200, { endpoint: j.endpoint, url: `http://127.0.0.1:${PORT}`, cost: 100, reqnum: 1, signature: "sig", request_idx: j.request_idx, __request_id: "x" });
  }

  if (url.pathname === "/generate/sync") {
    const wf = j.payload.input.workflow_json;
    genBodies.push({ auth_data: j.auth_data, wf, request_id: j.payload.input.request_id });
    const prompt = Object.values(wf).find((n) => n.class_type === "CLIPTextEncode" && n._meta?.title === "Positive")?.inputs.text || "";
    const local = Object.values(wf).some((n) => n.class_type === "AIVSSaveVideoBase64");
    if (prompt.includes("SLOW")) await new Promise((r) => setTimeout(r, 6000));
    if (prompt.includes("FAIL")) return send(res, 200, { status: "failed", message: "CUDA out of memory", output: [], comfyui_response: { execution_details: { error: "OOM" } } });
    if (prompt.includes("NONODE") && local) return send(res, 200, { status: "failed", message: "Prompt outputs failed validation: missing_node_type AIVSSaveVideoBase64 does not exist", output: [] });
    if (local) {
      return send(res, 200, { status: "completed", output: [], comfyui_response: { pid: { outputs: { 9999: { aivs_video_b64: [MP4.toString("base64")] } } } } });
    }
    return send(res, 200, { status: "completed", output: [{ filename: "ComfyUI_0001.mp4", url: `http://127.0.0.1:${PORT}/file.mp4` }], comfyui_response: {} });
  }

  if (url.pathname.startsWith("/api/") && auth === "Bearer bad") return send(res, 401, { success: false, error: "unauthorized" });
  if (url.pathname === "/api/v0/endptjobs/" && req.method === "GET")
    return send(res, 200, { success: true, results: [{ id: 7, endpoint_name: "comfyui-json", cold_workers: 0, min_load: 0, max_workers: 3, endpoint_state: "active" }] });
  if (url.pathname.startsWith("/api/v0/endptjobs/")) return send(res, 200, { success: true });
  if (url.pathname === "/api/v0/workergroups/" && req.method === "GET")
    return send(res, 200, { results: [{ id: 9, endpoint_id: 7, endpoint_name: "comfyui-json", search_params: "verified=true rentable=true rented=false gpu_name in [RTX_3090]", template_hash: "oldhash" }] });
  if (url.pathname.startsWith("/api/v0/workergroups/")) return send(res, 200, { success: true });
  if (url.pathname === "/api/v0/bundles/")
    return send(res, 200, { offers: [
      { gpu_name: "RTX_4090", dph_total: 0.45, gpu_ram: 24564 }, { gpu_name: "RTX_4090", dph_total: 0.6, gpu_ram: 24564 },
      { gpu_name: "RTX_5090", dph_total: 0.8, gpu_ram: 32607 }, { gpu_name: "L40S", dph_total: 0.7, gpu_ram: 46068 }] });
  if (url.pathname === "/api/v0/template/" && req.method === "GET")
    return send(res, 200, { templates: [{ id: 1, hash_id: "oldhash", name: "Wan 2.2 14B t2v (Serverless)", image: "vastai/comfy", tag: "v1", env: "-e A=1", onstart: "#!/bin/bash\nentrypoint.sh\n", runtype: "ssh", recommended_disk_space: 120, private: false }] });
  if (url.pathname === "/api/v0/template/" && req.method === "POST") return send(res, 200, { success: true, template: { id: 2, hash_id: "newhash123" } });
  send(res, 404, { error: "not found " + url.pathname });
}).listen(PORT, "127.0.0.1", () => console.log("mock up", PORT));
