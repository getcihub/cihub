package web

import (
	"context"
	"database/sql"
	"net/http"
	"time"

	"github.com/dchest/uniuri"
	"github.com/drone/go-login/login"
	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

// period at which the user account is synchronized
// with the remote system. Default is weekly.
var syncPeriod = time.Hour * 24 * 7

// period at which the sync should timeout
var syncTimeout = time.Minute * 30

// HandleLogin creates an http.HandlerFunc that handles user
// authentication and session initialization.
func HandleLogin(
	users core.UserStore,
	userz core.UserService,
	session core.Session,
	syncer core.Syncer,
) http.HandlerFunc {
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
				Syncing: true,
				Synced:  0,
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

		// If the user account has never been synchronized we
		// execute the synchronization logic.
		if time.Unix(user.Synced, 0).Add(syncPeriod).Before(time.Now()) {
			user.Syncing = true
		}

		err = users.Update(ctx, user)
		if err != nil {
			// if the account update fails we should still
			// proceed to create the user session. This is
			// considered a non-fatal error.
			logger.WithError(err).Errorln("web: cannot update user")
		}

		// launch the synchronization process in a go-routine,
		// since it is a long-running process and can take up
		// to a few minutes.
		if user.Syncing {
			go synchronize(syncer, user)
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

func synchronize(syncer core.Syncer, user *core.User) {
	log := logrus.WithField("login", user.Login)
	log.Debugf("begin synchronization")

	timeout, cancel := context.WithTimeout(context.Background(), syncTimeout)
	timeout = logger.WithContext(timeout, log)
	defer cancel()

	_, err := syncer.Sync(timeout, user)
	if err != nil {
		log.Debugf("synchronization failed: %s", err)
	} else {
		log.Debugf("synchronization success")
	}
}

func writeLoginError(w http.ResponseWriter, r *http.Request, err error) {
	http.Redirect(w, r, "/login?error="+err.Error(), http.StatusSeeOther)
}
