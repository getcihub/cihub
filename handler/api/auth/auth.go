package auth

import (
	"net/http"

	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/request"
	"github.com/getcihub/cihub/logger"
)

// HandleAuthentication returns an http.HandlerFunc middleware that
// authenticates the http.Request and errors if the account cannot be authenticated.
func HandleAuthentication(session core.Session) func(http.Handler) http.Handler {
	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			ctx := r.Context()
			log := logger.FromContext(ctx)
			user, err := session.Get(r)

			// this block of code checks the error message and user
			// returned from the session, including some edge cases,
			// to prevent a session from being falsely created.
			if err != nil || user == nil || user.ID == 0 {
				log.Debugln("api: guest access")
				next.ServeHTTP(w, r)
				return
			}

			log = log.WithFields(
				logrus.Fields{
					"user.active": user.Active,
					"user.admin":  user.Admin,
					"user.login":  user.Login,
				},
			)

			ctx = logger.WithContext(ctx, log)
			next.ServeHTTP(w, r.WithContext(
				request.WithUser(ctx, user),
			))
		})
	}
}
