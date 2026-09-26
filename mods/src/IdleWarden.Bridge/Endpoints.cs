// SPDX-License-Identifier: Apache-2.0

using System;
using System.IO;
using System.IO.Pipes;
using System.Net;
using System.Net.Sockets;
using System.Runtime.InteropServices;
using System.Text;

namespace IdleWarden.Bridge
{
    /// <summary>
    /// Where a mod waits for the host. A named pipe on Windows, a Unix domain
    /// socket elsewhere, at the paths <c>crates/bridge</c> connects to.
    /// </summary>
    public interface IBridgeEndpoint : IDisposable
    {
        string Description { get; }

        /// <summary>Blocks until the host connects, then owns the stream.</summary>
        Stream Accept();
    }

    public static class Endpoints
    {
        /// <summary>The longest path a `sockaddr_un` can carry, terminator aside.</summary>
        public const int LongestSocketPath = 107;

        public static bool OnWindows => RuntimeInformation.IsOSPlatform(OSPlatform.Windows);

        public static string PipeName(string endpointName)
        {
            return "idlewarden." + endpointName;
        }

        public static string SocketPath(string endpointName)
        {
            var directory = Environment.GetEnvironmentVariable("XDG_RUNTIME_DIR");
            if (string.IsNullOrEmpty(directory))
            {
                directory = "/tmp";
            }

            return directory.TrimEnd('/') + "/idlewarden." + endpointName + ".sock";
        }

        public static IBridgeEndpoint ForThisPlatform(string endpointName)
        {
            return OnWindows
                ? (IBridgeEndpoint)new PipeEndpoint(PipeName(endpointName))
                : new SocketEndpoint(SocketPath(endpointName));
        }
    }

    internal sealed class PipeEndpoint : IBridgeEndpoint
    {
        private readonly string name;
        private NamedPipeServerStream waiting;

        internal PipeEndpoint(string name)
        {
            this.name = name;
        }

        public string Description => @"\\.\pipe\" + name;

        /// Asynchronous, although nothing here awaits: closing a synchronous pipe
        /// does not cancel a pending wait for a host, it waits for it. Under the
        /// Mono runtime Unity ships, that wait happens on the main thread while the
        /// game quits, and the game never closes. An asynchronous pipe has its
        /// pending wait cancelled when it is closed.
        public Stream Accept()
        {
            var pipe = new NamedPipeServerStream(
                name,
                PipeDirection.InOut,
                1,
                PipeTransmissionMode.Byte,
                PipeOptions.Asynchronous);

            waiting = pipe;
            try
            {
                pipe.WaitForConnection();
                return pipe;
            }
            catch
            {
                pipe.Dispose();
                throw;
            }
        }

        public void Dispose()
        {
            waiting?.Dispose();
        }
    }

    internal sealed class SocketEndpoint : IBridgeEndpoint
    {
        private readonly string path;
        private readonly Socket listener;

        internal SocketEndpoint(string path)
        {
            if (Encoding.UTF8.GetByteCount(path) > Endpoints.LongestSocketPath)
            {
                throw new ArgumentException(
                    "a socket path is at most " + Endpoints.LongestSocketPath + " bytes: " + path,
                    nameof(path));
            }

            this.path = path;
            Forget();

            listener = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
            listener.Bind(new UnixEndPoint(path));
            listener.Listen(1);
        }

        public string Description => path;

        public Stream Accept()
        {
            return new NetworkStream(listener.Accept(), ownsSocket: true);
        }

        public void Dispose()
        {
            try
            {
                listener.Dispose();
            }
            finally
            {
                Forget();
            }
        }

        private void Forget()
        {
            try
            {
                if (File.Exists(path))
                {
                    File.Delete(path);
                }
            }
            catch (IOException)
            {
            }
            catch (UnauthorizedAccessException)
            {
            }
        }
    }

    /// <summary>
    /// `UnixDomainSocketEndPoint` arrived after netstandard2.0, and a mod is
    /// built against netstandard2.0 to load in Unity, so the `sockaddr_un` is
    /// written out here instead.
    /// </summary>
    internal sealed class UnixEndPoint : EndPoint
    {
        private const int FamilyBytes = 2;

        private readonly string path;

        internal UnixEndPoint(string path)
        {
            this.path = path ?? throw new ArgumentNullException(nameof(path));
        }

        public override AddressFamily AddressFamily => AddressFamily.Unix;

        public override SocketAddress Serialize()
        {
            var bytes = Encoding.UTF8.GetBytes(path);
            var address = new SocketAddress(AddressFamily.Unix, FamilyBytes + bytes.Length + 1);
            for (var at = 0; at < bytes.Length; at++)
            {
                address[FamilyBytes + at] = bytes[at];
            }

            address[FamilyBytes + bytes.Length] = 0;
            return address;
        }

        public override EndPoint Create(SocketAddress socketAddress)
        {
            if (socketAddress == null)
            {
                throw new ArgumentNullException(nameof(socketAddress));
            }

            var length = 0;
            var bytes = new byte[Math.Max(0, socketAddress.Size - FamilyBytes)];
            for (var at = 0; at < bytes.Length; at++)
            {
                bytes[at] = socketAddress[FamilyBytes + at];
                if (bytes[at] == 0)
                {
                    break;
                }

                length++;
            }

            return new UnixEndPoint(Encoding.UTF8.GetString(bytes, 0, length));
        }

        public override string ToString()
        {
            return path;
        }
    }
}
