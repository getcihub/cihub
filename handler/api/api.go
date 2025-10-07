package api

import (
	"net/http"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/acl"
	"github.com/getcihub/cihub/handler/api/auth"
	"github.com/getcihub/cihub/handler/api/user"
	"github.com/getcihub/cihub/handler/api/users"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
)

// Server is a http.Handler exposing CIHub functionality over HTTP.
type Server struct {
	Session core.Session
	Users   core.UserStore
}

func New(session core.Session, users core.UserStore) Server {
	return Server{
		Session: session,
		Users:   users,
	}
}

// Handler returns an http.Handler.
func (s Server) Handler() http.Handler {
	r := chi.NewRouter()

	r.Use(middleware.Recoverer)
	r.Use(middleware.NoCache)
	r.Use(auth.HandleAuthentication(s.Session))

	r.Route("/users", func(r chi.Router) {
		r.Use(acl.AuthorizeAdmin)
		r.Get("/", users.HandleList(s.Users))
		r.Get("/{login}", users.HandleFind(s.Users))
	})

	r.Route("/user", func(r chi.Router) {
		r.Use(acl.AuthorizeUser)
		r.Get("/", user.HandleFind())
	})

	return r
}
