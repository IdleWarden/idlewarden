// SPDX-License-Identifier: MPL-2.0

using System;
using System.Collections.Generic;
using System.IO;
using System.Net.Sockets;
using System.Text;
using System.Threading;
using IdleWarden.Bridge;
using Xunit;

namespace IdleWarden.Bridge.Tests
{
    public class EndpointTests
    {
        private sealed class Counter : IGameBridge
        {
            public string PluginId => "dev.example.game";

            public string ApiVersion => "^0.1";

            public IReadOnlyList<Signal> Observe()
            {
                return new List<Signal> { new Signal("resource.gold", Value.Int(7)) };
            }

            public ActionOutcome Act(Intent intent)
            {
                return ActionOutcome.Succeeded;
            }
        }

        [Fact]
        public void APipeLivesInTheNamespaceTheHostOpens()
        {
            Assert.Equal("idlewarden.reference", Endpoints.PipeName("reference"));
        }

        [Fact]
        public void ASocketLivesWhereTheHostLooksForIt()
        {
            var runtime = Environment.GetEnvironmentVariable("XDG_RUNTIME_DIR");
            try
            {
                Environment.SetEnvironmentVariable("XDG_RUNTIME_DIR", "/run/user/1000/");
                Assert.Equal(
                    "/run/user/1000/idlewarden.reference.sock",
                    Endpoints.SocketPath("reference"));

                Environment.SetEnvironmentVariable("XDG_RUNTIME_DIR", null);
                Assert.Equal("/tmp/idlewarden.reference.sock", Endpoints.SocketPath("reference"));
            }
            finally
            {
                Environment.SetEnvironmentVariable("XDG_RUNTIME_DIR", runtime);
            }
        }

        [Fact]
        public void ASocketPathTooLongForSockaddrIsRefused()
        {
            if (Endpoints.OnWindows)
            {
                return;
            }

            var runtime = Environment.GetEnvironmentVariable("XDG_RUNTIME_DIR");
            try
            {
                Environment.SetEnvironmentVariable("XDG_RUNTIME_DIR", "/tmp/" + new string('d', 120));
                Assert.Throws<ArgumentException>(() => Endpoints.ForThisPlatform("reference").Dispose());
            }
            finally
            {
                Environment.SetEnvironmentVariable("XDG_RUNTIME_DIR", runtime);
            }
        }

        [Fact]
        public void TheSameModAnswersOverASocketWhereThereAreNoPipes()
        {
            if (Endpoints.OnWindows)
            {
                return;
            }

            var name = "test-" + Environment.ProcessId;
            var path = Endpoints.SocketPath(name);
            using var server = new BridgeServer(name, new Counter(), _ => { });
            server.Start();

            var pumping = true;
            var pump = new Thread(() =>
            {
                while (Volatile.Read(ref pumping))
                {
                    server.Pump();
                    Thread.Sleep(5);
                }
            });
            pump.IsBackground = true;
            pump.Start();

            try
            {
                using var client = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
                Connect(client, path);

                using var stream = new NetworkStream(client, ownsSocket: false);
                using var writer = new StreamWriter(stream, new UTF8Encoding(false)) { AutoFlush = true, NewLine = "\n" };
                using var reader = new StreamReader(stream, new UTF8Encoding(false));

                writer.WriteLine("{\"request\":\"hello\",\"api_version\":\"0.1.0\"}");
                var hello = reader.ReadLine();
                Assert.Contains("dev.example.game", hello);

                writer.WriteLine("{\"request\":\"observe\"}");
                var observed = reader.ReadLine();
                Assert.Contains("resource.gold", observed);

                writer.WriteLine("{\"request\":\"act\",\"intent\":{\"name\":\"collect\"}}");
                var acted = reader.ReadLine();
                Assert.Contains("succeeded", acted);
            }
            finally
            {
                Volatile.Write(ref pumping, false);
                pump.Join(TimeSpan.FromSeconds(2));
            }
        }

        [Fact]
        public void ClosingThePipeWhileItAwaitsAHostReturnsPromptly()
        {
            if (!Endpoints.OnWindows)
            {
                return;
            }

            var endpoint = Endpoints.ForThisPlatform("closing-" + Environment.ProcessId);
            var awaiting = new Thread(() =>
            {
                try
                {
                    endpoint.Accept();
                }
                catch (Exception)
                {
                }
            }) { IsBackground = true };
            awaiting.Start();
            Thread.Sleep(300);

            // Disposed from its own thread so a hang fails this test instead of
            // freezing the runner. In the game this call happens on Unity's main
            // thread as it quits, and a hang there is a game that never closes.
            var closing = new Thread(endpoint.Dispose) { IsBackground = true };
            closing.Start();

            Assert.True(
                closing.Join(TimeSpan.FromSeconds(3)),
                "closing the pipe blocked on the pending wait for a host, which freezes the game on exit");
            Assert.True(awaiting.Join(TimeSpan.FromSeconds(3)), "the wait for a host never gave up");
        }

        [Fact]
        public void TheSocketFileGoesAwayWithTheServer()
        {
            if (Endpoints.OnWindows)
            {
                return;
            }

            var name = "gone-" + Environment.ProcessId;
            var path = Endpoints.SocketPath(name);

            var endpoint = Endpoints.ForThisPlatform(name);
            Assert.True(File.Exists(path), path + " was never bound");

            endpoint.Dispose();
            Assert.False(File.Exists(path), path + " outlived the mod");
        }

        private static void Connect(Socket client, string path)
        {
            var deadline = DateTime.UtcNow.AddSeconds(5);
            while (true)
            {
                try
                {
                    client.Connect(new UnixDomainSocketEndPoint(path));
                    return;
                }
                catch (SocketException) when (DateTime.UtcNow < deadline)
                {
                    Thread.Sleep(25);
                }
            }
        }
    }
}
