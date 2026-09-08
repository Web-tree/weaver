# acme-api

A small HTTP service that keeps the acme product catalogue in sync with downstream
inventory systems. Written in TypeScript on Node 20.

Contributors — human and AI — are expected to read this file before touching code.

## Manual Additions

> Hand-written notes that Weaver must preserve across runs.

- Before opening a PR, post a preview link in `#eng-reviews`.
- Never log customer email addresses, even at debug level.
- The staging database is rebuilt nightly; treat anything in `main` as fair game for the next rebuild.
