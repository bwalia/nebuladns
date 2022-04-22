<template>
    <nuxt-link class="primary-button" :to="link">
        <div class="primary-button-wrapper">
            <slot></slot>
        </div>

    </nuxt-link>
</template>

<script>
export default {
    props: ['link']
}
</script>

<style lang="css">

@import "~/assets/css/variables.css";

.primary-button {
    display: inline-block;
    background: var(--tint);
    line-height: 56px;
    color: var(--default);
    text-decoration: none;
    user-select: none;
    padding: 0 1.5em;
    border-radius: 20px;
    font-weight: bold;
}

.primary-button:last-child {
  background: var(--foil);
  color: #fff;
}

.primary-button .primary-button-wrapper {
    display: flex;
    align-items: center;
}

.primary-button svg {
  width: 18px;
  height: 18px;
  transform: translateY(-1px);
  margin-right: 5px;
}

.primary-button:active { box-shadow: 0 0 0 5px rgba(57, 123, 170, 0.33); }


</style>