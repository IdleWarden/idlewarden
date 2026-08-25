// SPDX-License-Identifier: Apache-2.0

using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Pipes;
using System.Text;
using System.Text.RegularExpressions;
using System.Threading;

namespace IdleWarden.Bridge
{
    /// <summary>
    /// The two methods a mod actually has to write. Both are called on the
    /// game's main thread, so Unity APIs are safe to touch inside them.
    /// </summary>
    public interface IGameBridge
    {
        /// <summary>The plugin this mod serves, matching its registry entry.</summary>
        string PluginId { get; }

        /// <summary>Host versions this mod works with, as a semver requirement.</summary>
        string ApiVersion { get; }

        /// <summary>
        /// Read the game state. Omit a signal you are not sure of rather than
        /// reporting a doubtful one: the Core stamps every bridged signal as
        /// certain and there is no way to say otherwise.
        /// </summary>
        IReadOnlyList<Signal> Observe();

        ActionOutcome Act(Intent intent);
    }

    /// <summary>
    /// Hosts <see cref="IGameBridge"/> on a named pipe.
    /// <para>
    /// Pipe IO runs on its own thread, but Unity objects may only be touched
    /// from the main thread, so requests are queued and executed by
    /// <see cref="Pump"/>. Call it once per frame from <c>Update</c>.
    /// </para>
    /// </summary>
    public sealed class BridgeServer : IDisposable
    {
        private static readonly Regex ValidEndpoint = new Regex("^[a-z0-9-]{1,64}$", RegexOptions.Compiled);
        private static readonly TimeSpan DefaultMainThreadTimeout = TimeSpan.FromSeconds(5);

        private readonly string pipeName;
        private readonly IGameBridge bridge;
        private readonly Action<string> log;
        private readonly TimeSpan mainThreadTimeout;
        private readonly string pluginId;
        private readonly string apiVersion;
        private readonly Queue<Job> pending = new Queue<Job>();
        private readonly object gate = new object();

        private Thread worker;
        private NamedPipeServerStream pipe;
        private volatile bool stopping;

        public BridgeServer(
            string endpointName,
            IGameBridge bridge,
            Action<string> log = null,
            TimeSpan? mainThreadTimeout = null)
        {
            if (endpointName == null)
            {
                throw new ArgumentNullException(nameof(endpointName));
            }

            if (!ValidEndpoint.IsMatch(endpointName))
            {
                throw new ArgumentException(
                    "an endpoint name is 1 to 64 characters of a-z, 0-9 and dashes",
                    nameof(endpointName));
            }

            this.bridge = bridge ?? throw new ArgumentNullException(nameof(bridge));
            this.log = log ?? (_ => { });
            this.mainThreadTimeout = mainThreadTimeout ?? DefaultMainThreadTimeout;

            pipeName = "idlewarden." + endpointName;
            pluginId = bridge.PluginId;
            apiVersion = bridge.ApiVersion;
        }

        public void Start()
        {
            if (worker != null)
            {
                throw new InvalidOperationException("this server is already started");
            }

            worker = new Thread(Serve) { IsBackground = true, Name = "idlewarden-bridge" };
            worker.Start();
            log("bridge listening on " + pipeName);
        }

        /// <summary>Executes queued requests. Call from the game's main thread.</summary>
        public void Pump()
        {
            while (true)
            {
                Job job;
                lock (gate)
                {
                    if (pending.Count == 0)
                    {
                        return;
                    }

                    job = pending.Dequeue();
                }

                if (job.Abandoned)
                {
                    continue;
                }

                try
                {
                    job.Result = job.Work();
                }
                catch (Exception error)
                {
                    job.Result = Response.Error(error.Message);
                }

                job.Done.Set();
            }
        }

        public void Dispose()
        {
            stopping = true;
            try
            {
                pipe?.Dispose();
            }
            catch (Exception error)
            {
                log("closing the pipe failed: " + error.Message);
            }

            worker?.Join(TimeSpan.FromSeconds(2));
        }

        private void Serve()
        {
            while (!stopping)
            {
                try
                {
                    using (pipe = new NamedPipeServerStream(
                        pipeName,
                        PipeDirection.InOut,
                        1,
                        PipeTransmissionMode.Byte,
                        PipeOptions.None))
                    {
                        pipe.WaitForConnection();
                        Converse(pipe);
                    }
                }
                catch (Exception error)
                {
                    if (!stopping)
                    {
                        log("bridge connection ended: " + error.Message);
                        Thread.Sleep(500);
                    }
                }
            }
        }

        private void Converse(Stream stream)
        {
            var encoding = new UTF8Encoding(false);
            using (var reader = new StreamReader(stream, encoding, false, 1024, true))
            using (var writer = new StreamWriter(stream, encoding, 1024, true) { AutoFlush = true, NewLine = "\n" })
            {
                var greeted = false;
                while (!stopping)
                {
                    var line = reader.ReadLine();
                    if (line == null)
                    {
                        log("the host disconnected");
                        return;
                    }

                    if (line.Length == 0)
                    {
                        continue;
                    }

                    writer.WriteLine(Answer(line, ref greeted));
                }
            }
        }

        private string Answer(string line, ref bool greeted)
        {
            Request request;
            try
            {
                request = Request.Parse(line);
            }
            catch (Exception error)
            {
                return Response.Error(error.Message);
            }

            if (request.Kind == RequestKind.Hello)
            {
                greeted = true;
                return Response.Hello(pluginId, apiVersion);
            }

            if (!greeted)
            {
                return Response.Error("say hello before anything else");
            }

            switch (request.Kind)
            {
                case RequestKind.Observe:
                    return OnMainThread(() => Response.Observed(bridge.Observe()));
                default:
                    var intent = request.Intent;
                    return OnMainThread(() => Response.Acted(bridge.Act(intent)));
            }
        }

        private string OnMainThread(Func<string> work)
        {
            var job = new Job(work);
            lock (gate)
            {
                pending.Enqueue(job);
            }

            if (job.Done.Wait(mainThreadTimeout))
            {
                return job.Result;
            }

            job.Abandoned = true;
            return Response.Error(
                "the game did not run a frame within " + (int)mainThreadTimeout.TotalMilliseconds + "ms");
        }

        private sealed class Job
        {
            internal Job(Func<string> work)
            {
                Work = work;
            }

            internal Func<string> Work { get; }

            internal string Result { get; set; }

            internal ManualResetEventSlim Done { get; } = new ManualResetEventSlim(false);

            internal volatile bool Abandoned;
        }
    }
}
