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

	// Containerd must be installed on machine
	ctr, err := containerd.New(config.Agent.Containerd, containerd.WithDefaultNamespace("cihub"))
	if err != nil {
		logger := logrus.WithError(err)
		logger.Fatalln("agent: cannot create containerd client")
	}

	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	manager := rpc.NewClient(
		config.RPC.Proto+"://"+config.RPC.Host,
		config.RPC.Secret,
	)

	imagez := image.New(ctr, config.Agent.Snapshotter)
	snapshotz := snapshot.New(ctr, config.Agent.Snapshotter)

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
			logger := logrus.WithError(err)
			logger.Errorln("agent: cannot ping the remote server")
			time.Sleep(time.Second)
		} else {
			logrus.Infoln("agent: successfully pinged the remote server")
			break
		}
	}

	// Launch agents for each pool
	var g errgroup.Group
	for _, pool := range config.Agent.Pools {
		pool := pool
		agent := &agent.Agent{
			Manager:     manager,
			Machine:     config.Agent.Name,
			Images:      imagez,
			Snapshots:   snapshotz,
			ID:          pool.ID,
			Firecracker: config.Agent.Firecracker,
			KernelArgs:  config.Agent.Kernel.Args,
			KernelPath:  config.Agent.Kernel.Path,
			Labels:      pool.Labels,
			Memory:      pool.Memory,
			OS:          pool.OS,
			VCPU:        pool.VCPU,
		}

		g.Go(func() error {
			logrus.
				WithField("capacity", pool.Capacity).
				WithField("labels", pool.Labels).
				WithField("machine", config.Agent.Name).
				WithField("memory", pool.Memory).
				WithField("pool", pool.ID).
				WithField("server", config.RPC.Proto+"://"+config.RPC.Host).
				WithField("vcpu", pool.VCPU).
				Infoln("agent: start polling remote server")
			return agent.Start(ctx, pool.Capacity)
		})
	}

	if err := g.Wait(); err != nil {
		logrus.WithError(err).Fatalln("program terminated")
	}
}
