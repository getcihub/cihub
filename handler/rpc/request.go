package rpc

import (
	"context"
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
)

// requestTimeout is the default http request timeout
var requestTimeout = time.Second * 10

// HandleRequest returns an http.HandlerFunc that processes an
// http.Request to request a runner from the queue for execution.
//
// POST /rpc/v1/request
func HandleRequest(scheduler core.Scheduler) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()
		ctx, cancel := context.WithTimeout(ctx, requestTimeout)
		defer cancel()

		machine, _ := MachineFrom(ctx)

		runner, err := scheduler.Request(ctx, machine)
		if err != nil {
			writeError(w, err)
		} else {
			writeJSON(w, runner)
		}
	}
}
