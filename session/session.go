package session

import (
	"net/http"
	"strings"
	"time"

	"github.com/dchest/authcookie"

	"github.com/getcihub/cihub/core"
)

// New returns a new cookie-based session management.
func New(users core.UserStore, config Config) core.Session {
	return &session{
		secret:  []byte(config.Secret),
		secure:  config.Secure,
		timeout: config.Timeout,
		users:   users,
	}
}

type session struct {
	users   core.UserStore
	secret  []byte
	secure  bool
	timeout time.Duration
}

func (s *session) Create(w http.ResponseWriter, user *core.User) error {
	cookie := &http.Cookie{
		Name:     "_session_",
		Path:     "/",
		MaxAge:   2147483647,
		HttpOnly: true,
		Secure:   s.secure,
		Value:    authcookie.NewSinceNow(user.Login, s.timeout, s.secret),
	}
	w.Header().Add("Set-Cookie", cookie.String()+"; SameSite=lax")
	return nil
}

func (s *session) Delete(w http.ResponseWriter) error {
	w.Header().Add("Set-Cookie", "_session_=deleted; Path=/; Max-Age=0")
	return nil
}

func (s *session) Get(r *http.Request) (*core.User, error) {
	switch {
	case isBearer(r):
		return s.fromBearer(r)
	default:
		return s.fromCookie(r)
	}
}

func (s *session) fromBearer(r *http.Request) (*core.User, error) {
	bearer := r.Header.Get("Authorization")
	token := strings.TrimPrefix(bearer, "Bearer ")
	return s.users.FindToken(r.Context(), token)
}

func (s *session) fromCookie(r *http.Request) (*core.User, error) {
	cookie, err := r.Cookie("_session_")
	if err != nil {
		return nil, nil
	}
	login := authcookie.Login(cookie.Value, s.secret)
	if login == "" {
		return nil, nil
	}
	return s.users.FindLogin(r.Context(), login)
}

func isBearer(r *http.Request) bool {
	return hasAuthPrefix(r, "Bearer ")
}

func hasAuthPrefix(r *http.Request, prefix string) bool {
	return strings.HasPrefix(r.Header.Get("Authorization"), prefix)
}
