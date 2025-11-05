package rpc

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"

	"github.com/getcihub/cihub/core"
)

// Server is a http.Handler exposing CIHub agent RPC interface over HTTP.
type Server struct {
	Machines  core.MachineStore
	Runners   core.RunnerStore
	Runnerz   core.RunnerService
	Scheduler core.Scheduler
}

func New(
	machines core.MachineStore,
	runners core.RunnerStore,
	runnerz core.RunnerService,
	scheduler core.Scheduler,
) Server {
	return Server{
		Machines:  machines,
		Runners:   runners,
		Runnerz:   runnerz,
		Scheduler: scheduler,
	}
}

// Handler returns an http.Handler.
func (s Server) Handler() http.Handler {
	r := chi.NewRouter()

	r.Use(middleware.Recoverer)
	r.Use(middleware.NoCache)
	r.Use(HandleAuthentication(s.Machines))

	r.Post("/join", HandleJoin(s.Machines))
	r.Post("/leave", HandleLeave(s.Machines))
	r.Post("/ping", HandlePing(s.Machines))
	r.Post("/request", HandleRequest(s.Scheduler))
	r.Post("/accept", HandleAccept(s.Runners))
	r.Post("/register", HandleRegister(s.Runners, s.Runnerz))
	r.Post("/lock", HandleLock(s.Machines))
	r.Post("/unlock", HandleUnlock(s.Machines))
	r.Post("/watch", HandleWatch(s.Scheduler))

	return r
}
