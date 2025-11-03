package api

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/go-chi/cors"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/acl"
	"github.com/getcihub/cihub/handler/api/auth"
	"github.com/getcihub/cihub/handler/api/orgs"
	"github.com/getcihub/cihub/handler/api/orgs/jobs"
	"github.com/getcihub/cihub/handler/api/orgs/machines"
	"github.com/getcihub/cihub/handler/api/orgs/runners"
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
	Installations core.InstallationStore
	Installationz core.InstallationService
	Jobs          core.JobStore
	Machines      core.MachineStore
	Memberships   core.MembershipStore
	Runners       core.RunnerStore
	Scheduler     core.Scheduler
	Session       core.Session
	Syncer        core.Syncer
	Users         core.UserStore
	Userz         core.UserService
}

func New(
	installations core.InstallationStore,
	installationz core.InstallationService,
	jobs core.JobStore,
	machines core.MachineStore,
	memberships core.MembershipStore,
	runners core.RunnerStore,
	scheduler core.Scheduler,
	session core.Session,
	syncer core.Syncer,
	users core.UserStore,
	userz core.UserService,
) Server {
	return Server{
		Installations: installations,
		Installationz: installationz,
		Jobs:          jobs,
		Machines:      machines,
		Memberships:   memberships,
		Runners:       runners,
		Scheduler:     scheduler,
		Session:       session,
		Syncer:        syncer,
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
		r.With(acl.AuthorizeAdmin).Get("/", orgs.HandleList())

		r.Route("/{owner}", func(r chi.Router) {
			r.Use(acl.InjectOrganization(s.Installations, s.Installationz, s.Memberships))
			r.Use(acl.CheckMember())

			r.Get("/", orgs.HandleFind())

			r.Route("/jobs", func(r chi.Router) {
				r.Get("/", jobs.HandleList(s.Jobs))
				r.Get("/{id}", jobs.HandleFind(s.Jobs))
			})

			r.Route("/machines", func(r chi.Router) {
				r.Get("/", machines.HandleList(s.Machines))
				r.With(acl.CheckAdmin()).Post("/", machines.HandleCreate(s.Machines))
				r.Get("/{name}", machines.HandleFind(s.Machines))
				r.With(acl.CheckAdmin()).
					Delete("/{name}", machines.HandleDelete(s.Machines, s.Scheduler))
				r.With(acl.CheckAdmin()).Patch("/{name}", machines.HandleUpdate())
				r.Get("/{name}/runners", machines.HandleRunners())
			})

			r.Route("/runners", func(r chi.Router) {
				r.Get("/{name}", runners.HandleFind(s.Runners))
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
		r.Get("/installations", user.HandleInstallations(s.Installations))
		r.Post("/installations", user.HandleSync(s.Syncer, s.Installations))
	})

	return r
}
