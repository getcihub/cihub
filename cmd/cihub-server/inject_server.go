package main

import (
	"net/http"

	chiprometheus "github.com/766b/chi-prometheus"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/google/wire"
	"github.com/palantir/go-githubapp/githubapp"
	"github.com/unrolled/secure"

	"github.com/getcihub/cihub/cmd/cihub-server/config"
	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api"
	"github.com/getcihub/cihub/handler/health"
	"github.com/getcihub/cihub/handler/web"
	"github.com/getcihub/cihub/hook/job"
	"github.com/getcihub/cihub/metric"
	"github.com/getcihub/cihub/orchestrator/manager"
	"github.com/getcihub/cihub/orchestrator/manager/rpc"
	"github.com/getcihub/cihub/server"
)

type (
	healthzHandler http.Handler
	hookHandler    http.Handler
	pprofHandler   http.Handler
	rpcHandler     http.Handler
)

// wire set for loading the server.
//
//nolint:unused
var serverSet = wire.NewSet(
	manager.New,
	api.New,
	web.New,
	provideEventHandlers,
	provideHealthz,
	provideHook,
	providePprof,
	provideRPC,
	provideRouter,
	provideServer,
	provideServerOptions,
)

// provideHealthz is a Wire provider function
// that returns a healthcheck HTTP handler.
func provideHealthz() healthzHandler {
	return healthzHandler(health.New())
}

// provideHook is a Wire provider function
// that returns a webhook HTTP handler.
func provideHook(config *config.Config, handlers []githubapp.EventHandler) hookHandler {
	return hookHandler(githubapp.NewEventDispatcher(handlers, config.GitHub.App.WebhookSecret))
}

// providePprof is a Wire provider function
// that returns a pprof HTTP handler.
func providePprof(config *config.Config) pprofHandler {
	switch {
	case config.Server.Debug:
		return pprofHandler(middleware.Profiler())
	default:
		return pprofHandler(http.NotFoundHandler())
	}
}

// provideRPC is a Wire provider function that returns an RPC
// handler that exposes the runner manager to a remote agent.
func provideRPC(m core.RunnerManager, config *config.Config) rpcHandler {
	return rpcHandler(rpc.NewServer(m, config.RPC.Secret))
}

// provideRouter is a Wire provider function that returns
// a router that serves the provided handlers.
func provideRouter(api api.Server, web web.Server, healthz healthzHandler, hook hookHandler, pprof pprofHandler, rpc rpcHandler, config *config.Config) *chi.Mux {
	r := chi.NewRouter()

	m := chiprometheus.NewMiddleware("server")
	r.Use(m)

	r.Mount("/healthz", healthz)
	r.Mount("/metrics", metric.HandleMetrics(config.Metric.Secret))
	r.Mount("/api", api.Handler())
	r.Mount("/hook", hook)
	r.Mount("/rpc/v1", rpc)
	r.Mount("/", web.Handler())
	r.Mount("/debug", pprof)

	return r
}

// provideServer is a Wire provider function that returns an
// http server that is configured from the environment.
func provideServer(handler *chi.Mux, config *config.Config) *server.Server {
	return &server.Server{
		Acme:    config.Server.Acme,
		Addr:    config.Server.Addr,
		Cert:    config.Server.Cert,
		Email:   config.Server.Email,
		Key:     config.Server.Key,
		Host:    config.Server.Host,
		Handler: handler,
	}
}

// provideServerOptions is a Wire provider function that returns
// the http web server security option from the environment.
func provideServerOptions(config *config.Config) secure.Options {
	return secure.Options{
		AllowedHosts:          config.HTTP.AllowedHosts,
		HostsProxyHeaders:     config.HTTP.HostsProxyHeaders,
		SSLRedirect:           config.HTTP.SSLRedirect,
		SSLTemporaryRedirect:  config.HTTP.SSLTemporaryRedirect,
		SSLHost:               config.HTTP.SSLHost,
		SSLProxyHeaders:       config.HTTP.SSLProxyHeaders,
		STSSeconds:            config.HTTP.STSSeconds,
		STSIncludeSubdomains:  config.HTTP.STSIncludeSubdomains,
		STSPreload:            config.HTTP.STSPreload,
		ForceSTSHeader:        config.HTTP.ForceSTSHeader,
		FrameDeny:             config.HTTP.FrameDeny,
		ContentTypeNosniff:    config.HTTP.ContentTypeNosniff,
		BrowserXssFilter:      config.HTTP.BrowserXSSFilter,
		ContentSecurityPolicy: config.HTTP.ContentSecurityPolicy,
		ReferrerPolicy:        config.HTTP.ReferrerPolicy,
	}
}

// provideEventHandlers is a Wire provider function that returns
// a list of GitHub webhook event handlers.
func provideEventHandlers(jobs core.JobStore, runners core.RunnerStore, scheduler core.Scheduler) []githubapp.EventHandler {
	return []githubapp.EventHandler{
		job.New(jobs, runners, scheduler),
	}
}
