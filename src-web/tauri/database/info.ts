export interface ConnectionInfo {
    items: ConnectionInfoItem[]
}

export interface ConnectionInfoItem {
    name: string
    value: ConnectionInfoValue
}

export type ConnectionInfoValue =
    | ConnectionInfoValueText
    | ConnectionInfoValueFile
    | ConnectionInfoValueUrl
    | ConnectionInfoValueServer

export const enum ConnectionInfoValueKind {
    Text = 'text',
    File = 'file',
    Url = 'url',
    Server = 'server'
}

export type ConnectionInfoValueText = { kind: ConnectionInfoValueKind.Text; text: string }
export type ConnectionInfoValueFile = { kind: ConnectionInfoValueKind.File; path: string }
export type ConnectionInfoValueUrl = { kind: ConnectionInfoValueKind.Url; url: string }
export type ConnectionInfoValueServer = {
    kind: ConnectionInfoValueKind.Server
    protocol: string
    server: string
}
