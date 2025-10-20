package main

import (
	"time"

	"github.com/containerd/containerd"
	"github.com/google/wire"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/cmd/cihub-server/config"
	"github.com/getcihub/cihub/orchestrator/agent"
	"github.com/getcihub/cihub/orchestrator/manager"
	"github.com/getcihub/cihub/service/image"
	"github.com/getcihub/cihub/service/snapshot"
)

// wire set for loading an agent.
var agentSet = wire.NewSet(
	provideAgent,
)

// provideAgent is a Wire provider function that returns a
// local agent runner configured from the environment.
func provideAgent(manager manager.RunnerManager, config *config.Config) *agent.Agent {
	// the local agent is only created when remote agents are disabled
	if !config.Agent.Enabled {
		return nil
	}

	opts := []containerd.ClientOpt{
		containerd.WithDefaultNamespace("cihub"),
		containerd.WithTimeout(time.Second * 5),
	}

	client, err := containerd.New(config.Agent.Containerd, opts...)
	if err != nil {
		logrus.WithError(err).
			Fatalln("cannot create containerd client")
		return nil
	}

	return &agent.Agent{
		Manager:   manager,
		Images:    image.New(client, config.Agent.Snapshotter),
		Snapshots: snapshot.New(client, config.Agent.Snapshotter),
		Kernel:    "/home/ubuntu/cihub/vmlinux",
		Labels:    config.Agent.Labels,
		Machine:   config.Agent.Name,
		Memory:    config.Agent.Memory,
		OS:        config.Agent.OS,
		VCPU:      config.Agent.VCPU,
	}
}
