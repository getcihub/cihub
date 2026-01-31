package api

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/go-chi/cors"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/acl"
	"github.com/getcihub/cihub/handler/api/auth"
	"github.com/getcihub/cihub/handler/api/installations"
	"github.com/getcihub/cihub/handler/api/installations/machines"
	"github.com/getcihub/cihub/handler/api/installations/runners"
	"github.com/getcihub/cihub/handler/api/user"
	"github.com/getcihub/cihub/handler/api/users"
	"github.com/getcihub/cihub/logger"
)

var corsOpts = cors.Options{
	AllowedOrigins:   []string{"*"},
	AllowedMethods:   []string{"GET", "POST", "PATCH", "PUT", "DELETE", "OPTIONS"},
	AllowedHeaders:   []string{"Accept", "Authorization", "Content-Type", "X-CSRF-Token"},
	ExposedHeaders:   []string{"Link"},
	AllowCredentials: true,
	MaxAge:           300,
}

// Server is a http.Handler exposing CIHub functionality over HTTP.
type Server struct {
	Installationz core.InstallationService
	Machines      core.MachineStore
	Runners       core.RunnerStore
	Runnerz       core.RunnerService
	Scheduler     core.Scheduler
	Session       core.Session
	System        *core.System
	Users         core.UserStore
	Userz         core.UserService
}

func New(
	installationz core.InstallationService,
	machines core.MachineStore,
	runners core.RunnerStore,
	runnerz core.RunnerService,
	scheduler core.Scheduler,
	session core.Session,
	system *core.System,
	users core.UserStore,
	userz core.UserService,
) Server {
	return Server{
		Installationz: installationz,
		Machines:      machines,
		Runners:       runners,
		Runnerz:       runnerz,
		Scheduler:     scheduler,
		Session:       session,
		System:        system,
		Users:         users,
		Userz:         userz,
	}
}

// Handler returns an http.Handler.
func (s Server) Handler() http.Handler {
	r := chi.NewRouter()

	r.Use(middleware.Recoverer)
	r.Use(middleware.NoCache)
	r.Use(middleware.RealIP)
	r.Use(auth.HandleAuthentication(s.Session))
	r.Use(logger.Middleware)

	cors := cors.New(corsOpts)
	r.Use(cors.Handler)

	r.Route("/installations", func(r chi.Router) {
		r.With(acl.AuthorizeAdmin).Get("/", installations.HandleList())

		r.Route("/{owner}", func(r chi.Router) {
			r.Use(acl.CheckAccess(s.Installationz, false))

			r.Route("/machines", func(r chi.Router) {
				r.Get("/", machines.HandleList(s.Machines))
				r.Post("/", machines.HandleCreate(s.Machines))
				r.Get("/{name}", machines.HandleFind(s.Machines))
				r.Delete("/{name}", machines.HandleDelete(s.Machines, s.Runners, s.Scheduler))
				r.Patch("/{name}", machines.HandleUpdate(s.Machines))
				r.Get("/{name}/runners", machines.HandleRunners(s.Machines, s.Runners))
			})

			r.Route("/runners", func(r chi.Router) {
				r.Get("/", runners.HandleList(s.Runners))
				r.Get("/{name}", runners.HandleFind(s.Runners))
				r.Delete("/{name}", runners.HandleCancel(s.Runners, s.Runnerz, s.Scheduler))
			})
		})
	})

	r.Route("/users", func(r chi.Router) {
		r.Use(acl.AuthorizeAdmin)
		r.Get("/", users.HandleList(s.Users))
		r.Get("/{login}", users.HandleFind(s.Users))
	})

	r.Route("/user", func(r chi.Router) {
		r.Use(acl.AuthorizeUser)
		r.Get("/", user.HandleFind())
		r.Patch("/", user.HandleUpdate(s.Users))
		r.Get("/emails", user.HandleEmails(s.Userz))
		r.Get("/installations", user.HandleInstallations(s.Installationz))
	})

	r.Get("/varz", HandleVarz(s.System))

	return r
}
