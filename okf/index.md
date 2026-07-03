---
okf_version: "0.1"
title: Loopless — Execution Runtime for AI Agents
description: >-
  Loopless is an execution runtime that sits between AI agents and their tools,
  observing every tool call and deciding whether execution should continue,
  adapt, or stop. Powered by the Microloop detection engine.
---

# Loopless Knowledge Bundle

This is the Open Knowledge Format (OKF) bundle for the Loopless project.
It describes the architecture, components, configuration, and usage of
the Loopless execution runtime for AI agents.

## Directory Structure

* [Overview](concepts/overview.md) — What Loopless is and why it exists
* [Architecture](concepts/architecture.md) — System architecture and data flow
* [Components](components/) — Core runtime components
* [Detection](detection/) — Loop detection engine (Microloop-powered)
* [Configuration](configuration/) — YAML policy and configuration
* [Adapters](adapters/) — Framework integrations
* [CLI](cli/) — Command-line interface
* [Integrations](integrations/) — Python API and external integrations
