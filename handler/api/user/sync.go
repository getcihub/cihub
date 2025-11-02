package user

import (
	"net/http"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/handler/api/request"
	"github.com/getcihub/cihub/logger"
)

// HandleSync returns an http.HandlerFunc synchronizes and then
// write a json-encoded list of installations to the response body.
func HandleSync(syncer core.Syncer, installations core.InstallationStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		viewer, _ := request.UserFrom(r.Context())

		_, err := syncer.Sync(r.Context(), viewer)
		if err != nil {
			render.InternalErrorf(w, err.Error())
			logger.FromRequest(r).WithError(err).
				Warnln("api: cannot synchronize account")
			return
		}

		result, err := installations.List(r.Context(), viewer)
		if err != nil {
			render.InternalErrorf(w, err.Error())
			logger.FromRequest(r).WithError(err).
				Warnln("api: cannot synchronize account")
		} else {
			render.OK(w, render.ReasonListed, result)
		}
	}
}
