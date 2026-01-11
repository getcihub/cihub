<p align="center">
  <a href="https://cihub.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://github.com/user-attachments/assets/5279ffbf-5b13-409e-b01e-354fd3f987d6" alt="CIHub" width="256" height="128" />
  </a>
</p>

<p align="center">
  <strong>Supercharged GitHub Actions Runners</strong><br>
  Run GitHub Actions on your own hardware — fast, secure, and fully isolated.<br/>
  <a href="https://cihub.io">Visit Website</a> • <a href="https://github.com/getcihub/cihub/issues">Report Issue</a> • <a href="https://github.com/getcihub/cihub/discussions">Discussions</a>
</p>

<p align="center">
  <img alt="CIHub Dashboard" src="docs/src/assets/screenshot.png">
</p>

> ⚠️ **This project is under heavy development.** The API, configuration, and behavior may change significantly. Use at your own risk and expect breaking changes between releases.

---

## What Is CIHub?

**CIHub** is an open-source, self-hosted GitHub Actions runner orchestrator that delivers **enterprise-grade isolation, speed, and control**.

Unlike traditional shared runners, CIHub launches **dedicated micro-VMs** (using [Firecracker](https://firecracker-microvm.github.io/)) for each workflow job — ensuring complete isolation, predictable performance, and security.


## Features

- **Isolation by Design**: Every job runs in its own ephemeral Firecracker micro-VM. No shared state, no side effects — just a clean sandbox.
- **Lightning-Fast Execution**: Micro-VMs start in milliseconds. Combined with your hardware, jobs run significantly faster than cloud runners.
- **Infinitely Scalable**: Start with a single machine or grow to a distributed cluster. Orchestrator handles load balancing and scheduling.
- **Multi-Architecture Support**: Support for both `amd64` and `arm64` architectures with automatic platform detection.
- **Zero-Friction Integration**: Drop-in replacement for GitHub Actions runners. Use standard workflow syntax with simple labels:
```yaml
jobs:
  test:
    runs-on: cihub-2cpu-4gb-amd64
```

## Architecture

CIHub consists of two main components:

1. **Server** – Central coordinator that receives GitHub webhooks, manages runner assignment, handles job scheduling, exposes REST APIs, and serves the management UI
2. **Agent** – Runs on each worker node to manage local Firecracker micro-VMs and execute assigned jobs

Both components communicate via REST APIs. The server handles all orchestration logic, while agents operate autonomously on their assigned nodes.

## Security

We take security seriously. If you discover a vulnerability:

**Please report it responsibly to [contact@cihub.io](mailto:contact@cihub.io)** instead of using public issues.

We will acknowledge receipt within 24 hours and provide updates as fixes are developed.

## Contributing

We welcome contributions! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

## License

Licensed under the [Mozilla Public License 2.0](LICENSE.md)
