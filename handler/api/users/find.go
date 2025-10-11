package users

import (
	"net/http"
	"strconv"

	"github.com/go-chi/chi/v5"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
)

// HandleFind returns an http.HandlerFunc that writes json-encoded
// user account information to the response body.
func HandleFind(users core.UserStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()
		login := chi.URLParam(r, "login")

		user, err := users.FindLogin(ctx, login)
		if err != nil {
			// the client can make a user request by providing
			// the user id as opposed to the username. If a
			// numeric user id is provided as input, attempt
			// to lookup the user by id.
			if id, _ := strconv.ParseInt(login, 10, 64); id != 0 {
				user, err = users.Find(ctx, id)
				if err == nil {
					render.OK(w, render.ReasonResolved, user)
					return
				}
			}

			render.NotFoundWithReason(w, render.ReasonUserNotFound)
			logger.FromRequest(r).Debugln("api: cannot find user")
		} else {
			render.OK(w, render.ReasonResolved, user)
		}
	}
}
