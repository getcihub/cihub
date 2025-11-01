package web

import (
	"net/http"

	"github.com/drone/go-login/login"
	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/unrolled/secure"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/web/dist"
)

type Server struct {
	Login   login.Middleware
	Options secure.Options
	Session core.Session
	Users   core.UserStore
	Userz   core.UserService
}

// Server is a http.Handler which exposes CIHub UI over HTTP.
func New(
	login login.Middleware,
	options secure.Options,
	session core.Session,
	users core.UserStore,
	userz core.UserService,
) Server {
	return Server{
		Login:   login,
		Options: options,
		Session: session,
		Users:   users,
		Userz:   userz,
	}
}

// Handler returns an http.Handler
func (s Server) Handler() http.Handler {
	r := chi.NewRouter()

	r.Use(middleware.Recoverer)
	r.Use(middleware.NoCache)
	r.Use(middleware.StripSlashes)

	security := secure.New(s.Options)
	r.Use(security.Handler)

	r.Route("/auth", func(r chi.Router) {
		r.Get("/logout", HandleLogout(s.Session))
		r.Post("/logout", HandleLogout(s.Session))
		r.Handle("/login", s.Login.Handler(http.HandlerFunc(HandleLogin(s.Users, s.Userz, s.Session))))
	})

	h := http.FileServer(dist.New())
	h = setupCache(h)
	r.Handle("/assets/*", h)
	r.NotFound(HandleIndex(s.Session))

	return r
}
