package server

import (
	"context"
	"crypto/tls"
	"net/http"
	"os"
	"path/filepath"
	"time"

	"golang.org/x/crypto/acme/autocert"
	"golang.org/x/sync/errgroup"
)

const timeoutGracefulShutdown = 5 * time.Second

// Server defines parameters for running an HTTP server.
type Server struct {
	Acme    bool
	Email   string
	Addr    string
	Cert    string
	Key     string
	Host    string
	Handler http.Handler
}

// ListenAndServe starts an HTTP server to respond to HTTP
// requests until provided context is canceled.
func (s Server) ListenAndServe(ctx context.Context) (err error) {
	if s.Acme {
		err = s.listenAndServeAcme(ctx)
	} else if s.Key != "" {
		err = s.listenAndServeTLS(ctx)
	} else {
		err = s.listenAndServe(ctx)
	}

	if err == http.ErrServerClosed {
		err = nil
	}
	return err
}

func (s Server) listenAndServe(ctx context.Context) error {
	g := errgroup.Group{}
	s1 := &http.Server{
		Addr:    s.Addr,
		Handler: s.Handler,
	}

	g.Go(s1.ListenAndServe)
	g.Go(func() error {
		<-ctx.Done()

		shutdown, cancel := context.WithTimeout(context.Background(), timeoutGracefulShutdown)
		defer cancel()

		return s1.Shutdown(shutdown)
	})

	return g.Wait()
}

func (s Server) listenAndServeAcme(ctx context.Context) error {
	g := errgroup.Group{}

	c := cacheDir()
	m := &autocert.Manager{
		Email:      s.Email,
		Cache:      autocert.DirCache(c),
		Prompt:     autocert.AcceptTOS,
		HostPolicy: autocert.HostWhitelist(s.Host),
	}

	s1 := &http.Server{
		Addr:    ":http",
		Handler: m.HTTPHandler(s.Handler),
	}
	s2 := &http.Server{
		Addr:    ":https",
		Handler: s.Handler,
		TLSConfig: &tls.Config{
			GetCertificate: m.GetCertificate,
			NextProtos:     []string{"h2", "http/1.1"},
			MinVersion:     tls.VersionTLS12,
		},
	}

	g.Go(s1.ListenAndServe)
	g.Go(func() error {
		return s2.ListenAndServeTLS("", "")
	})

	g.Go(func() error {
		<-ctx.Done()

		var g errgroup.Group
		shutdown, cancel := context.WithTimeout(context.Background(), timeoutGracefulShutdown)
		defer cancel()

		g.Go(func() error { return s1.Shutdown(shutdown) })
		g.Go(func() error { return s2.Shutdown(shutdown) })

		return g.Wait()
	})

	return g.Wait()
}

func (s Server) listenAndServeTLS(ctx context.Context) error {
	g := errgroup.Group{}

	s1 := &http.Server{
		Addr:    ":http",
		Handler: http.HandlerFunc(redirect),
	}
	s2 := &http.Server{
		Addr:    ":https",
		Handler: s.Handler,
	}

	g.Go(s1.ListenAndServe)
	g.Go(func() error {
		return s2.ListenAndServeTLS(s.Cert, s.Key)
	})

	g.Go(func() error {
		<-ctx.Done()

		var g errgroup.Group
		shutdown, cancel := context.WithTimeout(context.Background(), timeoutGracefulShutdown)
		defer cancel()

		g.Go(func() error {
			return s1.Shutdown(shutdown)
		})
		g.Go(func() error {
			return s2.Shutdown(shutdown)
		})

		return g.Wait()
	})

	return g.Wait()
}

func cacheDir() string {
	const base = "golang-autocert"
	if xdg := os.Getenv("XDG_CACHE_HOME"); xdg != "" {
		return filepath.Join(xdg, base)
	}
	return filepath.Join(os.Getenv("HOME"), ".cache", base)
}

func redirect(w http.ResponseWriter, req *http.Request) {
	target := "https://" + req.Host + req.URL.Path
	http.Redirect(w, req, target, http.StatusTemporaryRedirect)
}
