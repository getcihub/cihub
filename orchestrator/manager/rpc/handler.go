package rpc

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
	"github.com/getcihub/cihub/store/shared/db"
)

// default http request timeout
var defaultTimeout = time.Second * 10

// HandlePing returns an http.HandlerFunc that makes an
// http.Request to ping the server and confirm connectivity.
//
// POST /rpc/v1/ping
func HandlePing() http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		writeOK(w) // this is a no-op
	}
}

// HandleRequest returns an http.HandlerFunc that processes an
// http.Request to request a runner from the queue for execution.
//
// POST /rpc/v1/request
func HandleRequest(m core.RunnerManager) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()
		ctx, cancel := context.WithTimeout(ctx, defaultTimeout)
		defer cancel()

		params := &core.Filter{}
		err := json.NewDecoder(r.Body).Decode(params)
		if err != nil {
			logger.FromRequest(r).
				WithError(err).
				Debugln("manager: cannot decode request payload")
			writeError(w, err)
			return
		}

		runner, err := m.Request(ctx, params)
		if err != nil {
			writeError(w, err)
		} else {
			writeJSON(w, runner)
		}
	}
}

// HandleAccept returns an http.HandlerFunc that processes an
// http.Request to accept ownership of the runner.
//
// POST /rpc/v1/accept
func HandleAccept(m core.RunnerManager) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		in := &acceptRequest{}
		err := json.NewDecoder(r.Body).Decode(in)
		if err != nil {
			logger.FromRequest(r).
				WithError(err).
				Debugln("manager: cannot decode accept request payload")
			writeError(w, err)
			return
		}

		err = m.Accept(context.Background(), in.Name, in.Machine)
		if err != nil {
			logger.FromRequest(r).
				WithError(err).
				Debugln("manager: cannot accept runner")
			writeError(w, err)
		} else {
			writeOK(w)
		}
	}
}

// HandleRegister returns an http.HandlerFunc that processes an
// http.Request to register GitHub runner and get details
//
// POST /rpc/v1/register
func HandleRegister(m core.RunnerManager) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		in := &registerRequest{}
		err := json.NewDecoder(r.Body).Decode(in)
		if err != nil {
			logger.FromRequest(r).
				WithError(err).
				Debugln("manager: cannot decode registering request payload")
			writeError(w, err)
			return
		}

		runner, err := m.Register(r.Context(), in.Name)
		if err != nil {
			logger.FromRequest(r).
				WithError(err).
				Debugln("manager: cannot register runner")
			writeError(w, err)
			return
		}

		json.NewEncoder(w).Encode(runner)
	}
}

// HandleWatch returns an http.HandlerFunc that accepts a
// blocking http.Request that watches a build for cancellation
// events.
//
// GET /rpc/v1/watch
func HandleWatch(m core.RunnerManager) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()
		ctx, cancel := context.WithTimeout(ctx, defaultTimeout)
		defer cancel()

		in := &watchRequest{}
		err := json.NewDecoder(r.Body).Decode(in)
		if err != nil {
			logger.FromRequest(r).
				WithError(err).
				Debugln("manager: cannot decode watch request payload")
			writeError(w, err)
			return
		}

		done, err := m.Watch(ctx, in.Name)
		if err != nil {
			logger.FromRequest(r).
				WithError(err).
				Debugln("manager: cannot watch runner cancellation")
			writeError(w, err)
			return
		}

		json.NewEncoder(w).Encode(&watchResponse{
			Done: done,
		})
	}
}

// write a 200 Status OK to the response body.
func writeJSON(w http.ResponseWriter, v interface{}) {
	json.NewEncoder(w).Encode(v)
}

// write a 200 Status OK to the response body.
func writeOK(w http.ResponseWriter) {
	w.WriteHeader(http.StatusOK)
}

// write an error message to the response body.
func writeError(w http.ResponseWriter, err error) {
	if errors.Is(err, context.DeadlineExceeded) {
		w.WriteHeader(524) // should retry
	} else if errors.Is(err, context.Canceled) {
		w.WriteHeader(524) // should retry
	} else if errors.Is(err, db.ErrOptimisticLock) {
		w.WriteHeader(409) // should abort
	} else {
		w.WriteHeader(400) // should fail
	}

	io.WriteString(w, err.Error())
}
