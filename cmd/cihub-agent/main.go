package main

import (
	"context"
	"flag"
	"os/signal"
	"syscall"
	"time"

	"github.com/containerd/containerd"
	"github.com/sirupsen/logrus"
	"golang.org/x/sync/errgroup"

	"github.com/getcihub/cihub/cmd/cihub-agent/config"
	"github.com/getcihub/cihub/orchestrator/agent"
	"github.com/getcihub/cihub/orchestrator/manager/rpc"
	"github.com/getcihub/cihub/service/image"
	"github.com/getcihub/cihub/service/snapshot"
)

func main() {
	var confpath string
	flag.StringVar(&confpath, "c", "./config.toml", "Path to the configuration file")
	flag.Parse()

	config, err := config.Load(confpath)
	if err != nil {
		logrus.Fatalln(err)
	}

	// Set logging level
	logrus.SetLevel(config.Logger.Level)

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	manager := rpc.NewClient(
		config.RPC.Proto+"://"+config.RPC.Host,
		config.RPC.Secret,
	)

	opts := []containerd.ClientOpt{
		containerd.WithDefaultNamespace("cihub"),
		containerd.WithTimeout(time.Second * 5),
	}

	client, err := containerd.New(config.Agent.Containerd, opts...)
	if err != nil {
		logrus.WithError(err).
			Fatalln("agent: cannot create containerd client")
	}

	agent := &agent.Agent{
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

	// Ping the server and block until a successful connection
	// to the server has been established.
	for {
		err := manager.Ping(ctx, config.Agent.Name)
		select {
		case <-ctx.Done():
			return
		default:
		}

		if ctx.Err() != nil {
			break
		}

		if err != nil {
			logrus.WithError(err).
				Errorln("agent: cannot ping the remote server")
			time.Sleep(time.Second)
		} else {
			logrus.Infoln("agent: successfully pinged the remote server")
			break
		}
	}

	var g errgroup.Group
	g.Go(func() error {
		logrus.
			WithField("name", config.Agent.Name).
			WithField("capacity", config.Agent.Capacity).
			WithField("server", config.RPC.Proto+"://"+config.RPC.Host).
			Infoln("start polling remote server")
		return agent.Start(ctx, config.Agent.Capacity)
	})

	if err := g.Wait(); err != nil {
		logrus.WithError(err).Fatalln("program terminated")
	}
}
