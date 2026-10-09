/**
 * Source evaluated once inside each QuickJS VM. It is an expression that yields a function
 * `(host, admittedJson) => api`. `host` is the only host capability the guest can reach; the
 * prelude keeps it in a closure and never exposes it as a global.
 *
 * Host kinds: `host("call", id, toolName, argsJson)`, `host("attempt", path)` for an access to
 * a name that is not admitted, and `host("done", ok, payloadJson)`.
 */
export const SANDBOX_PRELUDE = `(function (host, admittedJson) {
  "use strict";
  var admitted = JSON.parse(admittedJson);
  var leaves = Object.create(null);
  var namespaces = Object.create(null);
  for (var i = 0; i < admitted.length; i++) {
    var segments = admitted[i].split(".");
    leaves[admitted[i]] = true;
    for (var j = 1; j < segments.length; j++) {
      namespaces[segments.slice(0, j).join(".")] = true;
    }
  }

  function fail(message) {
    throw new TypeError(message);
  }

  // Accept only plain JSON data: null, booleans, finite numbers, strings, arrays and plain
  // objects with data properties, without cycles. Anything else throws before it can cross.
  function assertJson(value, where, depth, stack) {
    if (value === null) return;
    var kind = typeof value;
    if (kind === "boolean" || kind === "string") return;
    if (kind === "number") {
      if (!isFinite(value)) fail(where + " is not a finite number");
      return;
    }
    if (kind !== "object") fail(where + " is not JSON (" + kind + ")");
    if (depth > 256) fail(where + " is nested too deeply");
    if (stack.indexOf(value) !== -1) fail(where + " is cyclic");
    var proto = Object.getPrototypeOf(value);
    stack.push(value);
    if (Array.isArray(value)) {
      if (proto !== Array.prototype) fail(where + " is not a plain array");
      for (var a = 0; a < value.length; a++) {
        if (!Object.prototype.hasOwnProperty.call(value, a)) fail(where + " has a hole");
        assertJson(value[a], where + "[" + a + "]", depth + 1, stack);
      }
    } else {
      if (proto !== Object.prototype && proto !== null) fail(where + " is not a plain object");
      var keys = Object.keys(value);
      for (var k = 0; k < keys.length; k++) {
        var desc = Object.getOwnPropertyDescriptor(value, keys[k]);
        if (!desc || !("value" in desc)) fail(where + "." + keys[k] + " is an accessor");
        assertJson(desc.value, where + "." + keys[k], depth + 1, stack);
      }
    }
    stack.pop();
  }

  var nextId = 1;
  var waiting = Object.create(null);

  function callTool(path, args) {
    if (args === undefined) args = {};
    assertJson(args, "arguments", 0, []);
    var id = nextId++;
    return new Promise(function (resolve, reject) {
      waiting[id] = { resolve: resolve, reject: reject };
      host("call", id, path, JSON.stringify(args));
    });
  }

  function settle(id, ok, payload) {
    var entry = waiting[id];
    if (!entry) return;
    delete waiting[id];
    var parsed = JSON.parse(payload);
    if (ok) {
      entry.resolve(parsed);
    } else {
      var error = new Error(String(parsed.message));
      error.name = "ToolError";
      error.kind = String(parsed.kind);
      entry.reject(error);
    }
  }

  var leafFns = Object.create(null);
  var nodes = Object.create(null);

  function leafFor(path) {
    if (!leafFns[path]) {
      leafFns[path] = function (args) {
        return callTool(path, args);
      };
    }
    return leafFns[path];
  }

  function nodeFor(path) {
    if (!nodes[path]) {
      nodes[path] = new Proxy(Object.create(null), {
        get: function (_target, key) {
          // Symbols and "then" are probes from the engine or from await, never tool access.
          if (typeof key !== "string" || key === "then") return undefined;
          var child = path === "" ? key : path + "." + key;
          if (leaves[child] === true) return leafFor(child);
          if (namespaces[child] === true) return nodeFor(child);
          host("attempt", child, 0, "");
          return undefined;
        },
        set: function () { return false; },
        defineProperty: function () { return false; },
        deleteProperty: function () { return false; },
        has: function () { return false; },
        ownKeys: function () { return []; },
        getPrototypeOf: function () { return null; },
        setPrototypeOf: function () { return false; },
        preventExtensions: function () { return false; }
      });
    }
    return nodes[path];
  }

  var tools = nodeFor("");

  function describeError(error) {
    var name = "Error";
    var message = "unprintable thrown value";
    try {
      if (error !== null && typeof error === "object") {
        name = String(error.name);
        message = String(error.message);
      } else {
        message = String(error);
      }
    } catch (ignored) {
      // Keep the fallbacks above.
    }
    return JSON.stringify({ name: name.slice(0, 256), message: message.slice(0, 4096) });
  }

  function serializeResult(value) {
    if (value === undefined) return "null";
    assertJson(value, "result", 0, []);
    return JSON.stringify(value);
  }

  function start(body) {
    var promise;
    try {
      promise = Promise.resolve(body(tools));
    } catch (error) {
      promise = Promise.reject(error);
    }
    promise.then(
      function (value) {
        var text;
        try {
          text = serializeResult(value);
        } catch (error) {
          var reason = error !== null && typeof error === "object" ? String(error.message) : String(error);
          host("done", false, JSON.stringify({ name: "TypeError", message: "result is not JSON: " + reason.slice(0, 4096) }));
          return;
        }
        host("done", true, text);
      },
      function (error) {
        host("done", false, describeError(error));
      }
    );
  }

  return { start: start, settle: settle };
})`;
