package user

import (
	"net/http"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/handler/api/request"
	"github.com/getcihub/cihub/logger"
)

// HandleEmails returns an http.HandlerFunc that write a json-encoded
// list of emails to the response body.
func HandleEmails(userz core.UserService) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		viewer, _ := request.UserFrom(r.Context())

		emails, err := userz.ListEmail(r.Context(), viewer)
		if err != nil {
			render.InternalError(w)
			logger.FromRequest(r).WithError(err).
				Debugln("api: cannot list user emails")
			return
		}

		render.OK(w, render.ReasonListed, emails)
	}
}
