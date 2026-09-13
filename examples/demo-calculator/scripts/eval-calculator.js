#!/usr/bin/env node
// scripts/eval-calculator.js — genuinely evaluates calculator.html rather
// than grepping for strings. Builds a minimal DOM stub around the page's
// inline <script>, clicks the buttons a real user would click for a couple
// of small expressions, and checks the displayed result. No dependencies,
// no network, deterministic.
//
// Exit codes: 0 = evaluation passed. 1 = evaluation ran but produced wrong
// results. 3 = no inline <script> found. 4 = the page's script threw while
// loading. 5 = could not find one of the required buttons at all (the
// static structural check in check.sh should already have caught this,
// this is a belt-and-braces guard).

"use strict";
const vm = require("node:vm");
const fs = require("node:fs");

const PAGE = process.argv[2] || "calculator.html";
const html = fs.readFileSync(PAGE, "utf8");

const scripts = [...html.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/gi)]
  .map((m) => m[1])
  .filter((s) => s.trim().length > 0)
  .join("\n;\n");

if (!scripts.trim()) {
  console.error("eval-calculator: no inline <script> found in " + PAGE);
  process.exit(3);
}

function parseAttrs(str) {
  const attrs = {};
  const attrRe = /([a-zA-Z0-9_-]+)\s*=\s*"([^"]*)"/g;
  let m;
  while ((m = attrRe.exec(str))) attrs[m[1].toLowerCase()] = m[2];
  return attrs;
}

function parseButtons(htmlSrc) {
  const out = [];
  const buttonRe = /<button\b([^>]*)>([\s\S]*?)<\/button>/gi;
  let m;
  while ((m = buttonRe.exec(htmlSrc))) {
    out.push({ attrs: parseAttrs(m[1]), text: m[2].replace(/<[^>]*>/g, "").trim() });
  }
  return out;
}

function makeElement(attrs, text) {
  const listeners = {};
  const el = {
    id: attrs.id || "",
    className: attrs.class || "",
    dataset: {},
    value: attrs.value !== undefined ? attrs.value : "",
    style: {},
    _text: text || "",
    _onclickRaw: attrs.onclick || null,
    _onclickFn: null,
    get textContent() {
      return this._text;
    },
    set textContent(v) {
      this._text = v;
    },
    get innerText() {
      return this._text;
    },
    set innerText(v) {
      this._text = v;
    },
    get innerHTML() {
      return this._text;
    },
    set innerHTML(v) {
      this._text = v;
    },
    set onclick(fn) {
      this._onclickFn = fn;
    },
    get onclick() {
      return this._onclickFn;
    },
    addEventListener(type, fn) {
      (listeners[type] = listeners[type] || []).push(fn);
    },
    removeEventListener() {},
    setAttribute(k, v) {
      attrs[k] = v;
    },
    getAttribute(k) {
      return attrs[k] === undefined ? null : attrs[k];
    },
    classList: {
      add() {},
      remove() {},
      toggle() {},
      contains() {
        return false;
      },
    },
    appendChild() {},
    focus() {},
    click() {
      const ev = { target: this, preventDefault() {} };
      if (this._onclickRaw && globalThisRef) {
        vm.runInContext(this._onclickRaw, globalThisRef);
      }
      if (this._onclickFn) this._onclickFn(ev);
      (listeners.click || []).forEach((fn) => fn(ev));
    },
  };
  for (const k of Object.keys(attrs)) {
    if (k.startsWith("data-")) {
      el.dataset[k.slice(5).replace(/-([a-z])/g, (_, c) => c.toUpperCase())] = attrs[k];
    }
  }
  return el;
}

let globalThisRef = null;

function buildEnv() {
  const buttons = parseButtons(html).map((b) => makeElement(b.attrs, b.text));
  const byId = {};
  for (const b of buttons) if (b.id) byId[b.id] = b;

  const idRe = /\bid\s*=\s*"([^"]+)"/g;
  let im;
  while ((im = idRe.exec(html))) {
    if (!byId[im[1]]) byId[im[1]] = makeElement({ id: im[1] }, "");
  }

  const document = {
    getElementById(id) {
      return byId[id];
    },
    querySelectorAll(sel) {
      if (sel.trim().toLowerCase() === "button") return buttons;
      return [];
    },
    querySelector(sel) {
      if (sel.startsWith("#")) return byId[sel.slice(1)];
      return null;
    },
    addEventListener(type, fn) {
      if (type === "DOMContentLoaded") fn();
    },
    createElement() {
      return makeElement({}, "");
    },
    body: makeElement({}, ""),
  };

  const localStorageBacking = {};
  const context = {
    document,
    console,
    Math,
    parseInt,
    parseFloat,
    isNaN,
    String,
    Number,
    Array,
    Object,
    JSON,
    setTimeout: (fn) => fn(),
    clearTimeout() {},
    alert() {},
    navigator: {},
    localStorage: {
      getItem: (k) => (k in localStorageBacking ? localStorageBacking[k] : null),
      setItem: (k, v) => {
        localStorageBacking[k] = String(v);
      },
      removeItem: (k) => {
        delete localStorageBacking[k];
      },
    },
  };
  context.window = context;
  context.globalThis = context;
  vm.createContext(context);
  globalThisRef = context;

  try {
    vm.runInContext(scripts, context, { timeout: 2000 });
  } catch (e) {
    console.error("eval-calculator: script threw while loading: " + e.message);
    process.exit(4);
  }

  return { buttons, byId };
}

function findButtonByLabel(buttons, label) {
  return buttons.find((b) => b.textContent.trim() === label);
}

function readResult(byId) {
  for (const cand of ["display", "result", "screen", "output", "calc-display", "value"]) {
    if (byId[cand]) {
      const el = byId[cand];
      const v = el.value !== "" && el.value !== undefined ? el.value : el.textContent;
      if (v !== undefined && String(v).trim() !== "") return String(v).trim();
    }
  }
  for (const el of Object.values(byId)) {
    const t = String(el.textContent || "").trim();
    if (/^-?\d+(\.\d+)?$/.test(t)) return t;
  }
  return undefined;
}

const cases = [
  { seq: ["7", "+", "3", "="], expect: 10 },
  { seq: ["9", "-", "4", "="], expect: 5 },
];

const failures = [];
for (const c of cases) {
  const { buttons, byId } = buildEnv();
  try {
    for (const label of c.seq) {
      const btn = findButtonByLabel(buttons, label);
      if (!btn) {
        console.error(`eval-calculator: no button labeled ${JSON.stringify(label)} was found`);
        process.exit(5);
      }
      btn.click();
    }
    const got = readResult(byId);
    if (got === undefined) {
      failures.push(`expression ${c.seq.join(" ")}: no readable result element after clicking`);
      continue;
    }
    const gotNum = parseFloat(got);
    if (Number.isNaN(gotNum) || gotNum !== c.expect) {
      failures.push(`expression ${c.seq.join(" ")}: expected ${c.expect}, got ${got}`);
    }
  } catch (e) {
    failures.push(`expression ${c.seq.join(" ")}: ${e.message}`);
  }
}

if (failures.length) {
  console.error("eval-calculator: dynamic evaluation failed:");
  for (const f of failures) console.error("  - " + f);
  process.exit(1);
}

console.log(`eval-calculator: dynamic evaluation passed (${cases.length} expressions)`);
process.exit(0);
