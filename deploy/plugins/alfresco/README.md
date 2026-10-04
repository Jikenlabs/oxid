# Plugin d'Intégration Alfresco Share & Content Services pour Oxid

Ce plugin permet d'intégrer **Oxid** nativement au sein d'**Alfresco Share & Content Services** pour offrir une visualisation et une annotation ultra-rapides avec une consommation mémoire minimale.

---

## 🚀 Installation dans Alfresco

### 1. Copier la configuration WebExtension
Placez le fichier `share-config-custom.xml` dans le répertoire d'extension de votre serveur Alfresco Share :
```bash
cp share-config-custom.xml /usr/local/tomcat/shared/classes/alfresco/web-extension/
```

### 2. Déployer l'action JavaScript
Copiez `oxid-action.js` dans le dossier des assets personnalisés de Share :
```bash
cp oxid-action.js /usr/local/tomcat/webapps/share/js/oxid/
```

### 3. Configurer l'URL du cluster Oxid
Dans `alfresco-global.properties` ou variable d'environnement :
```properties
oxid.url=https://viewer.votre-domaine.com
```

### 4. Redémarrer Alfresco Share
Le bouton **"Visualiser avec Oxid"** apparaîtra automatiquement sur toutes les actions documentaires d'Alfresco Share.
