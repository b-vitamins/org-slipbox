#!/usr/bin/env node
// Copyright (C) 2026 Ayan Das
// SPDX-License-Identifier: GPL-3.0-or-later

import fs from "node:fs";

function abort(message) {
  console.error(`FAIL ${message}`);
  process.exitCode = 1;
}

function readJson(path) {
  try {
    return JSON.parse(fs.readFileSync(path, "utf8"));
  } catch (error) {
    throw new Error(`${path}: ${error.message}`);
  }
}

function valueAt(value, path) {
  return path.split(".").reduce((current, key) => current?.[key], value);
}

const arguments_ = process.argv.slice(2);
const observe = arguments_[0] === "--observe";
const reference = arguments_[0] === "--reference";
if (observe || reference) arguments_.shift();
if (arguments_.length < 2) {
  console.error("usage: check-performance-report.mjs [--observe|--reference] BUDGETS REPORT...");
  process.exit(2);
}

const [budgetPath, ...reportPaths] = arguments_;
const budgets = readJson(budgetPath);
if (budgets.schema !== 1) throw new Error(`${budgetPath}: unsupported schema ${budgets.schema}`);

const observations = new Map();
for (const reportPath of reportPaths) {
  const report = readJson(reportPath);
  const context = report.context ?? {};
  const build = context.build ?? {};
  if (reference) {
    if (build.model !== budgets.reference.model) {
      abort(`${reportPath}: model ${build.model ?? "absent"}, expected ${budgets.reference.model}`);
    }
    if ((build.version?.sdk ?? 0) < budgets.reference.minimumSdk) {
      abort(`${reportPath}: SDK ${build.version?.sdk ?? "absent"} is below ${budgets.reference.minimumSdk}`);
    }
    if (build.type !== budgets.reference.buildType) {
      abort(`${reportPath}: build type ${build.type ?? "absent"}, expected ${budgets.reference.buildType}`);
    }
  }
  for (const benchmark of report.benchmarks ?? []) {
    const className = benchmark.className?.split(".").at(-1);
    const key = `${className}.${benchmark.name}`;
    const entries = observations.get(key) ?? [];
    entries.push({ benchmark, reportPath });
    observations.set(key, entries);
  }
}

if (observe) {
  for (const [key, entries] of [...observations].sort()) {
    for (const { benchmark } of entries) {
      const values = [];
      for (const [name, metric] of Object.entries(benchmark.metrics ?? {})) {
        values.push(`${name}.median=${metric.median}`);
        values.push(`${name}.maximum=${metric.maximum}`);
      }
      for (const [name, metric] of Object.entries(benchmark.sampledMetrics ?? {})) {
        values.push(`${name}.P95=${metric.P95}`);
      }
      console.log(`OBSERVE ${key} iterations=${benchmark.repeatIterations} ${values.join(" ")}`);
    }
  }
  process.exit(0);
}

for (const [key, specification] of Object.entries(budgets.benchmarks)) {
  const entries = observations.get(key) ?? [];
  if (entries.length === 0) {
    abort(`${key}: no measurement`);
    continue;
  }
  for (const { benchmark, reportPath } of entries) {
    if ((benchmark.repeatIterations ?? 0) < budgets.reference.minimumIterations) {
      abort(`${key}: ${benchmark.repeatIterations ?? 0} iterations in ${reportPath}`);
    }
    for (const [metricPath, maximum] of Object.entries(specification.metrics)) {
      const measured = valueAt(benchmark, metricPath);
      if (typeof measured !== "number") {
        abort(`${key}: missing ${metricPath} in ${reportPath}`);
      } else if (measured > maximum) {
        abort(`${key}: ${metricPath}=${measured} exceeds ${maximum}`);
      } else {
        console.log(`PASS ${key} ${metricPath}=${measured} <= ${maximum}`);
      }
    }
  }
}

if (process.exitCode) process.exit(process.exitCode);
console.log(`PASS ${Object.keys(budgets.benchmarks).length} Android performance budgets`);
