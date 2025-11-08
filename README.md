<p align="center">
  <a href="https://cihub.io/?utm_source=github&utm_medium=logo" target="_blank">
    <img src="https://github.com/user-attachments/assets/5279ffbf-5b13-409e-b01e-354fd3f987d6" alt="CIHub" width="256" height="128" />
  </a>
</p>

<p align="center">
  <strong>Supercharged GitHub Actions Runners</strong><br>
  Run GitHub Actions on your own hardware — fast, secure, and fully isolated.
</p>

## 🚀 What Is CIHub?

**CIHub** lets you run your GitHub Actions workflows on your own servers, safely and efficiently.

Instead of sharing runners across jobs, CIHub launches **tiny, isolated virtual machines** (using Firecracker) for each workflow run.
That means:

* More **security** (each job runs in its own sandbox)
* Better **speed** (you control the hardware)
* Full **control** over your CI/CD environment

<p align="center">
  <img src="https://github.com/getcihub/cihub/raw/main/.github/screenshots/machines.png" width="270" />
  <img src="https://github.com/getcihub/cihub/raw/main/.github/screenshots/jobs.png" width="270" />
  <img src="https://github.com/getcihub/cihub/raw/main/.github/screenshots/job-details.png" width="270" />
</p>

## 💡 Why Use CIHub?

* **Isolation by design** – Every job runs in its own micro-VM.
* **Cost-effective** – Reuse your own servers instead of paying for hosted runners.
* **Plug-and-play** – Works with regular GitHub workflows, no syntax changes needed.
* **Observable** – Built-in metrics and logs so you can see what’s happening.
* **Scalable** – From a single laptop to a cluster of servers.

## ⚙️ How It Works

When a GitHub Action starts:

1. GitHub tells CIHub there’s a job to run.
2. CIHub picks a free machine (an *agent*).
3. The agent spins up a **micro-VM** using Firecracker.
4. The VM downloads your workflow code and runs it.
5. When the job finishes, the VM is destroyed.

Each job is fast, clean, and isolated — like a fresh container for every build.

## 🏁 Getting Started

1. **Install CIHub** on a Linux server.
2. **Connect it to GitHub** using a GitHub App (so it can receive job requests).
3. **Run the Agent** on your machines to handle builds.
4. **Add CIHub labels** to your workflows — and you’re done!

Example:

```yaml
jobs:
  test:
    runs-on: cihub-2cpu-4gb-amd64
    steps:
      ...
```

CIHub needs to be hosted on your own systems, and maintained on your end. If you do not feel like managing yet another service, you may use [CIHub cloud](https://cihub.io?utm_source=github&utm_medium=getting-started) instead.

## 📚 Learn More

* [Deployment Guide](docs/deployment.md) – How to set it up
* [Configuration Reference](docs/configuration.md) – All settings
* [Architecture Overview](docs/architecture.md) – How it’s built

## 🧾 License

Licensed under the [Mozilla Public License 2.0](LICENSE.md)

## 🛡️ Security

If you discover a vulnerability, please report it responsibly to **[contact@cihub.io](mailto:contact@cihub.io)**.
We take security seriously and will respond quickly.
