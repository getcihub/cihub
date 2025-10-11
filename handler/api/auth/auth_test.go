package auth

import (
	"database/sql"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/sirupsen/logrus"
	"go.uber.org/mock/gomock"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/request"
	"github.com/getcihub/cihub/mock"
)

func init() {
	logrus.SetOutput(io.Discard)
}

func TestAuth(t *testing.T) {
	controller := gomock.NewController(t)
	defer controller.Finish()

	mockUser := &core.User{
		ID:    1,
		Login: "octocat",
		Admin: true,
		Token: "$2a$04$rR2VvGjM9iqAAoyLSE4IrexAlxDbIS3M5YKtj9ANs7vraki0ybYJq 197XXbZablx0RPQ8",
	}

	session := mock.NewMockSession(controller)
	session.EXPECT().Get(gomock.Any()).Return(mockUser, nil)

	w := httptest.NewRecorder()
	r := httptest.NewRequest("GET", "/?access_token=VA.197XXbZablx0RPQ8", nil)

	HandleAuthentication(session)(
		http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// use dummy status code to signal the next handler in
			// the middleware chain was properly invoked.
			w.WriteHeader(http.StatusTeapot)

			// verify the user was added to the request context
			if user, _ := request.UserFrom(r.Context()); user != mockUser {
				t.Errorf("Expect user in context")
			}
		}),
	).ServeHTTP(w, r)

	if got, want := w.Code, http.StatusTeapot; got != want {
		t.Errorf("Want status code %d, got %d", want, got)
	}
}

func TestAuth_Guest(t *testing.T) {
	controller := gomock.NewController(t)
	defer controller.Finish()

	w := httptest.NewRecorder()
	r := httptest.NewRequest("GET", "/", nil)

	session := mock.NewMockSession(controller)
	session.EXPECT().Get(gomock.Any()).Return(nil, sql.ErrNoRows)

	HandleAuthentication(session)(
		http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// use dummy status code to signal the next handler in
			// the middleware chain was properly invoked.
			w.WriteHeader(http.StatusTeapot)

			// verify the user was added to the request context
			if _, ok := request.UserFrom(r.Context()); ok {
				t.Errorf("Expect guest mode, no user in context")
			}
		}),
	).ServeHTTP(w, r)

	if got, want := w.Code, http.StatusTeapot; got != want {
		t.Errorf("Want status code %d, got %d", want, got)
	}
}
