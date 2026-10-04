# Module d'Intégration SharePoint & WebDAV (Nextcloud, ownCloud) pour Oxid

Ce module permet d'intégrer la visionneuse **Oxid** dans **Microsoft SharePoint Online / On-Premise**, **Nextcloud** ou toute GED exposant un protocole WebDAV.

---

## 💻 Méthodes d'Intégration

### 1. Intégration via SPFx (SharePoint Framework WebPart)
1. Intégrez le script `oxid-viewer.es.js` dans votre solution SPFx :
   ```html
   <script type="module" src="https://viewer.domaine.com/oxid-viewer.es.js"></script>
   <oxid-viewer doc-id="{ItemId}" token="{BearerToken}"></oxid-viewer>
   ```
2. La visionneuse s'affiche de façon réactive dans n'importe quel composant de page moderne SharePoint.

### 2. Intégration Iframe / Page dédiée (SharePoint & Nextcloud)
- Utilisez `embed-oxid.html` en passant en paramètre d'URL :
  ```
  https://viewer.domaine.com/embed-oxid.html?fileUrl=https://sharepoint.com/sites/docs/contrat.pdf&token=eyJ...
  ```
