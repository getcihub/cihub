package web

import (
	"database/sql"
	"net/http"
	"time"

	"github.com/dchest/uniuri"
	"github.com/drone/go-login/login"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
)

// HandleLogin creates an http.HandlerFunc that handles user
// authentication and session initialization.
func HandleLogin(users core.UserStore, userz core.UserService, session core.Session) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()
		err := login.ErrorFrom(ctx)
		if err != nil {
			writeLoginError(w, r, err)
			logrus.WithError(err).
				Debugln("web: cannot create user")
			return
		}

		// The authorization token is passed from the
		// login middleware in the context.
		token := login.TokenFrom(ctx)

		account, err := userz.Find(ctx, token.Access, token.Refresh)
		if err != nil {
			writeLoginError(w, r, err)
			logrus.WithError(err).
				Debugln("web: cannot find remote user")
			return
		}

		logger := logrus.WithField("login", account.Login)
		logger.Debugln("web: attempting authentication")

		user, err := users.FindLogin(ctx, account.Login)
		if err == sql.ErrNoRows {
			user = &core.User{
				Login:   account.Login,
				Avatar:  account.Avatar,
				Admin:   false,
				Active:  true,
				Created: time.Now().Unix(),
				Updated: time.Now().Unix(),
				Access:  token.Access,
				Refresh: token.Refresh,
				Token:   uniuri.NewLen(32),
			}

			if !token.Expires.IsZero() {
				user.Expiry = token.Expires.Unix()
			}

			err = users.Create(ctx, user)
			if err != nil {
				writeLoginError(w, r, err)
				logger.WithError(err).
					Errorln("web:cannot create user")
				return
			}

			logger.Debugln("web: successfully created user")
		} else if err != nil {
			writeLoginError(w, r, err)
			logger.WithError(err).
				Errorln("web: cannot find user")
			return
		}

		user.Avatar = account.Avatar
		user.Access = token.Access
		user.Refresh = token.Refresh
		if !token.Expires.IsZero() {
			user.Expiry = token.Expires.Unix()
		}

		err = users.Update(ctx, user)
		if err != nil {
			// if the account update fails we should still
			// proceed to create the user session. This is
			// considered a non-fatal error.
			logger.WithError(err).Errorln("web: cannot update user")
		}

		redirect := "/"
		if len(user.Email) == 0 {
			redirect = "/register"
		}

		logger.Debugln("web: authentication successful")

		session.Create(w, user)
		http.Redirect(w, r, redirect, http.StatusSeeOther)
	}
}

func writeLoginError(w http.ResponseWriter, r *http.Request, err error) {
	http.Redirect(w, r, "/login?error="+err.Error(), http.StatusSeeOther)
}
