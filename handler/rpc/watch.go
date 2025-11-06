package rpc

import (
	"context"
	"encoding/json"
	"net/http"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
)

// HandleWatch returns an http.HandlerFunc that accepts a
// blocking http.Request that watches a build for cancellation
// events.
//
// GET /rpc/v1/watch
func HandleWatch(scheduler core.Scheduler) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()
		ctx, cancel := context.WithTimeout(ctx, requestTimeout)
		defer cancel()

		in := new(core.Runner)
		err := json.NewDecoder(r.Body).Decode(in)
		if err != nil {
			logger.FromRequest(r).
				WithError(err).
				Debugln("manager: cannot decode watch request payload")
			writeError(w, err)
			return
		}

		_, err = scheduler.Cancelled(ctx, in.Name)
		if err != nil {
			writeError(w, err)
		} else {
			writeOK(w)
		}
	}
}
