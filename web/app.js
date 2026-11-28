// Wire Desk — lightweight dashboard client (specs #216-218).
// Framework-free vanilla JS, works on mobile and desktop.

(function () {
  const BASE = window.location.origin;
  let token = null;
  let isSignup = false;

  const els = {
    login: document.getElementById("btnLogin"),
    signup: document.getElementById("btnSignup"),
    grade: document.getElementById("btnGrade"),
    docs: document.getElementById("btnDocs"),
    dashboard: document.getElementById("dashboard"),
    grader: document.getElementById("grader"),
    auth: document.getElementById("auth"),
    authForm: document.getElementById("authForm"),
    authTitle: document.getElementById("authTitle"),
    authSubmit: document.getElementById("authSubmit"),
    authToggle: document.getElementById("authToggle"),
    userInfo: document.getElementById("userInfo"),
    agentRunsList: document.getElementById("agentRunsList"),
    findingsList: document.getElementById("findingsList"),
    usageInfo: document.getElementById("usageInfo"),
    graderTarget: document.getElementById("graderTarget"),
    btnRunGrade: document.getElementById("btnRunGrade"),
    graderResult: document.getElementById("graderResult"),
    btnNewRun: document.getElementById("btnNewRun"),
    btnNewScan: document.getElementById("btnNewScan"),
    btnPlans: document.getElementById("btnPlans"),
  };

  function show(section) {
    [els.dashboard, els.grader, els.auth].forEach((s) => (s.style.display = "none"));
    section.style.display = "block";
  }

  async function api(method, path, body) {
    const headers = { "Content-Type": "application/json", "X-Request-Id": crypto.randomUUID() };
    if (token) headers.Authorization = "Bearer " + token;
    const res = await fetch(BASE + path, { method, headers, body: body ? JSON.stringify(body) : undefined });
    let json = null;
    try { json = await res.json(); } catch (_) {}
    if (!res.ok) throw new Error((json && json.error && json.error.message) || "HTTP " + res.status);
    return json;
  }

  async function loadDashboard() {
    try {
      const me = await api("GET", "/api/v1/auth/me");
      token = localStorage.getItem("wd_token") || null;
      if (!token) { show(els.auth); return; }
      const profile = await api("GET", "/api/v1/auth/me");
      els.userInfo.textContent = (profile.data && profile.data.email) || "user";
      const runs = await api("GET", "/api/v1/agents/runs");
      els.agentRunsList.textContent = JSON.stringify(runs.data || [], null, 2) || "no runs";
      const findings = await api("GET", "/api/v1/scans/:id/findings".replace(":id", ""));
      els.findingsList.textContent = JSON.stringify(findings.data || [], null, 2) || "no findings";
      const usage = await api("GET", "/api/v1/billing/usage");
      els.usageInfo.textContent = JSON.stringify(usage.data || {}, null, 2) || "no usage";
      show(els.dashboard);
    } catch (e) {
      show(els.auth);
    }
  }

  els.login.onclick = () => { isSignup = false; els.authTitle.textContent = "Sign in"; els.authToggle.textContent = "Need an account? Sign up"; show(els.auth); };
  els.signup.onclick = () => { isSignup = true; els.authTitle.textContent = "Create account"; els.authToggle.textContent = "Already have an account? Sign in"; show(els.auth); };
  els.authToggle.onclick = () => { isSignup = !isSignup; els.authTitle.textContent = isSignup ? "Create account" : "Sign in"; els.authToggle.textContent = isSignup ? "Already have an account? Sign in" : "Need an account? Sign up"; };

  els.authForm.onsubmit = async (e) => {
    e.preventDefault();
    const email = document.getElementById("authEmail").value;
    const password = document.getElementById("authPassword").value;
    const tenant = document.getElementById("authTenant").value;
    try {
      if (isSignup) {
        await api("POST", "/api/v1/auth/register", { email, password, tenant_name: tenant || email, tenant_slug: tenant });
      }
      const res = await api("POST", "/api/v1/auth/login", { email, password });
      token = res.data && res.data.token;
      if (token) { localStorage.setItem("wd_token", token); loadDashboard(); }
      else { alert("login returned no token"); }
    } catch (err) { alert(err.message); }
  };

  els.grade.onclick = () => show(els.grader);
  els.docs.onclick = () => alert("See API.md for the full endpoint reference.");

  els.btnRunGrade.onclick = async () => {
    const target = els.graderTarget.value;
    if (!target) return alert("enter a target");
    try {
      const res = await api("POST", "/api/v1/public/security-grade", { target });
      els.graderResult.textContent = JSON.stringify(res.data || res, null, 2);
      els.graderResult.classList.remove("hidden");
    } catch (err) { alert(err.message); }
  };

  els.btnNewRun.onclick = () => {
    const goal = prompt("Enter agent goal:");
    if (!goal) return;
    fetch(BASE + "/api/v1/agents/runs", {
      method: "POST", headers: { "Content-Type": "application/json", Authorization: "Bearer " + token, "X-Request-Id": crypto.randomUUID() },
      body: JSON.stringify({ goal, workspace_id: "demo" }),
    }).then(r => r.json()).then(j => alert(JSON.stringify(j))).catch(e => alert(e));
  };

  els.btnNewScan.onclick = () => {
    const target = prompt("Enter scan target:");
    if (!target) return;
    fetch(BASE + "/api/v1/scans", {
      method: "POST", headers: { "Content-Type": "application/json", Authorization: "Bearer " + token, "X-Request-Id": crypto.randomUUID() },
      body: JSON.stringify({ target, workspace_id: "demo", scan_type: "passive_recon" }),
    }).then(r => r.json()).then(j => alert(JSON.stringify(j))).catch(e => alert(e));
  };

  els.btnPlans.onclick = () => {
    api("GET", "/api/v1/billing/plans").then(j => alert(JSON.stringify(j.data || j, null, 2))).catch(e => alert(e));
  });

  // Boot
  if (localStorage.getItem("wd_token")) { token = localStorage.getItem("wd_token"); loadDashboard(); }
  else { show(els.auth); }
})();
