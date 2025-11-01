package api

import (
	"net/http"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/go-chi/cors"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/acl"
	"github.com/getcihub/cihub/handler/api/auth"
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
	Session core.Session
	Users   core.UserStore
	Userz   core.UserService
}

func New(
	session core.Session,
	users core.UserStore,
	userz core.UserService,
) Server {
	return Server{
		Session: session,
		Users:   users,
		Userz:   userz,
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
	})

	return r
}
