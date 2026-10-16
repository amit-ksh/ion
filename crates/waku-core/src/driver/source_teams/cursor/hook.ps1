& {
    process {
        $ErrorActionPreference = 'Stop'
        $IonClient = $null
        $IonHandler = $null
        try {
            $IonPayload = [string]$_
            if ([System.Text.Encoding]::UTF8.GetByteCount($IonPayload) -gt 45056) {
                throw 'Ion hook input exceeds its bound'
            }
            $IonToken = [Environment]::GetEnvironmentVariable('ION_CURSOR_TEAM_HOOK_TOKEN')
            if ($IonToken -notmatch '^[0-9a-fA-F]{64}$') {
                throw 'Ion hook token is unavailable'
            }
            Add-Type -AssemblyName System.Net.Http
            $IonHandler = [System.Net.Http.HttpClientHandler]::new()
            $IonHandler.UseProxy = $false
            $IonHandler.AllowAutoRedirect = $false
            $IonClient = [System.Net.Http.HttpClient]::new($IonHandler)
            $IonClient.Timeout = [TimeSpan]::FromSeconds(2)
            $IonClient.MaxResponseContentBufferSize = 65536
            $IonClient.DefaultRequestHeaders.Authorization = [System.Net.Http.Headers.AuthenticationHeaderValue]::new('Bearer', $IonToken)
            $IonBody = [System.Net.Http.StringContent]::new($IonPayload, [System.Text.Encoding]::UTF8, 'application/json')
            $IonBody.Headers.ContentType.CharSet = $null
            $IonUri = [System.UriBuilder]::new('http', '127.0.0.1', __ION_HOOK_PORT__, 'hook').Uri
            $IonResponse = $IonClient.PostAsync($IonUri, $IonBody).GetAwaiter().GetResult()
            if ([int]$IonResponse.StatusCode -ne 200) {
                throw 'Ion hook refused'
            }
            [Console]::WriteLine($IonResponse.Content.ReadAsStringAsync().GetAwaiter().GetResult())
        } catch {
            [Console]::WriteLine('{"permission":"deny","user_message":"Ion could not authorize this team request. Retry from the active task."}')
        } finally {
            if ($null -ne $IonClient) { $IonClient.Dispose() }
            if ($null -ne $IonHandler) { $IonHandler.Dispose() }
        }
        exit 0
    }
}
